//! 音声結合の進捗（0〜1 の割合）の計算と通知。Tauri・外部プロセスに依存しない。
//!
//! 結合は「取り出し → 区間生成 → 結合」の3段階で進む。全体 0〜1 を段階ごとの範囲（`Span`）に配分し、
//! 各段階の中の進み具合を範囲内の割合に変換して `ProgressReporter` へ渡す。

/// 0〜1 の割合を受け取る通知先。コマンド層で Channel への送信に変換する
pub type ProgressSink<'a> = &'a mut dyn FnMut(f64);

/// これ未満の増加は通知しない（通知の送りすぎを防ぐ）
const MIN_STEP: f64 = 0.01;
/// 浮動小数点の誤差で 1% ちょうどの増加を取りこぼさないための余裕
const EPSILON: f64 = 1e-9;

/// 取り出し（音声区間の書き出し＋ffprobe）に配分する割合。結合方式は取り出しの後に決まるため、両方式で共通にする
pub const EXTRACT_SHARE: f64 = 0.2;

/// 取り出しの後の残りを「区間生成」と「結合」に分ける比
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StageWeights {
    pub generate: f64,
    pub concat: f64,
}

impl StageWeights {
    /// copy 方式: 区間生成（無音区間の生成）0.1、結合 0.7
    pub const COPY: StageWeights = StageWeights {
        generate: 0.1,
        concat: 0.7,
    };
    /// 再エンコード方式: 区間生成（各区間の WAV 正規化＋ffprobe）0.5、結合 0.4
    pub const REENCODE: StageWeights = StageWeights {
        generate: 0.5,
        concat: 0.4,
    };

    /// `rest` をこの比で（区間生成, 結合）の範囲に分ける
    pub fn split(self, rest: Span) -> (Span, Span) {
        let [generate, concat] = rest.split([self.generate, self.concat]);
        (generate, concat)
    }
}

/// 全体の中の1段階分の範囲 [start, end]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Span {
    pub start: f64,
    pub end: f64,
}

impl Span {
    pub const FULL: Span = Span {
        start: 0.0,
        end: 1.0,
    };

    /// 範囲内の割合 `t`（0〜1。範囲外は丸め、NaN は 0）を全体の割合に変換する
    pub fn at(self, t: f64) -> f64 {
        let t = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) };
        self.start + (self.end - self.start) * t
    }

    /// 範囲を `weights` の比で分割する。最後の範囲の終わりは `self.end` に一致させる（誤差で 1 に届かないことを防ぐ）。
    /// 重みの合計が0以下・有限でない場合は等分する
    pub fn split<const N: usize>(self, weights: [f64; N]) -> [Span; N] {
        let total: f64 = weights.iter().sum();
        let valid = total.is_finite() && total > 0.0 && weights.iter().all(|w| *w >= 0.0);
        let mut acc = 0.0;
        let mut start = self.start;
        std::array::from_fn(|i| {
            let end = if i + 1 == N {
                self.end
            } else {
                acc += if valid { weights[i] } else { 1.0 };
                let ratio = if valid { acc / total } else { acc / N as f64 };
                self.at(ratio)
            };
            let span = Span { start, end };
            start = end;
            span
        })
    }
}

/// 完了数 ÷ 対象数（0〜1）。対象が0件なら段階を終えたものとして 1 を返す
pub fn fraction(done: usize, total: usize) -> f64 {
    if total == 0 {
        1.0
    } else {
        (done as f64 / total as f64).min(1.0)
    }
}

/// 出力済みの秒数 ÷ 総尺（0〜1）。総尺が0以下・有限でなければ、結合の終わりまで進められないため 0 を返す
pub fn time_fraction(out_sec: f64, total_sec: f64) -> f64 {
    if !total_sec.is_finite() || total_sec <= 0.0 || !out_sec.is_finite() {
        0.0
    } else {
        (out_sec / total_sec).clamp(0.0, 1.0)
    }
}

/// ffmpeg の `-progress` の1行から出力済みの秒数を取り出す。
/// `out_time_us=123456` → `Some(0.123456)`。負の値は 0 秒にする。別のキーや `N/A` は `None`
pub fn parse_out_time_sec(line: &str) -> Option<f64> {
    let value = line.trim().strip_prefix("out_time_us=")?;
    let us: i64 = value.trim().parse().ok()?;
    Some(us.max(0) as f64 / 1_000_000.0)
}

/// 単調化と間引きを行う通知器。
/// 送信済みの値以下の割合は無視し、1% 未満の増加は送らない。1 は必ず（1回だけ）送る
pub struct ProgressReporter<'a> {
    sink: ProgressSink<'a>,
    sent: f64,
}

impl<'a> ProgressReporter<'a> {
    pub fn new(sink: ProgressSink<'a>) -> Self {
        Self { sink, sent: 0.0 }
    }

    pub fn report(&mut self, ratio: f64) {
        if ratio.is_nan() {
            return;
        }
        let ratio = ratio.clamp(0.0, 1.0);
        let finished = ratio >= 1.0 && self.sent < 1.0;
        if finished || ratio - self.sent >= MIN_STEP - EPSILON {
            self.sent = ratio;
            (self.sink)(ratio);
        }
    }

    /// 送信済みの値（copy 結合の失敗から再エンコードで再試行するときの起点に使う）
    pub fn current(&self) -> f64 {
        self.sent
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1e-9,
            "actual {actual}, expected {expected}"
        );
    }

    /// `report` に順に渡した割合のうち、送られたものを返す
    fn reported(ratios: &[f64]) -> Vec<f64> {
        let mut sent = Vec::new();
        {
            let mut sink = |r| sent.push(r);
            let mut reporter = ProgressReporter::new(&mut sink);
            for &r in ratios {
                reporter.report(r);
            }
        }
        sent
    }

    #[test]
    fn parse_out_time_reads_microseconds() {
        assert_eq!(parse_out_time_sec("out_time_us=1500000"), Some(1.5));
        assert_eq!(parse_out_time_sec("out_time_us=0"), Some(0.0));
        // 改行・前後の空白は無視する
        assert_eq!(parse_out_time_sec("out_time_us=250000\r\n"), Some(0.25));
        // 出力開始直後の負の値は 0 秒とする
        assert_eq!(parse_out_time_sec("out_time_us=-23220"), Some(0.0));
    }

    #[test]
    fn parse_out_time_ignores_other_lines() {
        assert_eq!(parse_out_time_sec("out_time_us=N/A"), None);
        assert_eq!(parse_out_time_sec("out_time_ms=1500000"), None);
        assert_eq!(parse_out_time_sec("out_time=00:00:01.500000"), None);
        assert_eq!(parse_out_time_sec("progress=continue"), None);
        assert_eq!(parse_out_time_sec(""), None);
    }

    #[test]
    fn span_at_maps_and_clamps() {
        let span = Span {
            start: 0.2,
            end: 0.6,
        };
        assert_close(span.at(0.0), 0.2);
        assert_close(span.at(0.5), 0.4);
        assert_close(span.at(1.0), 0.6);
        assert_close(span.at(-1.0), 0.2);
        assert_close(span.at(2.0), 0.6);
        assert_close(span.at(f64::NAN), 0.2);
    }

    #[test]
    fn span_split_follows_weights_and_ends_exactly() {
        let [a, b, c] = Span::FULL.split([0.2, 0.1, 0.7]);
        assert_close(a.start, 0.0);
        assert_close(a.end, 0.2);
        assert_close(b.start, a.end);
        assert_close(b.end, 0.3);
        assert_close(c.start, b.end);
        assert_eq!(c.end, 1.0);

        // 部分範囲も比で分ける
        let [x, y] = Span {
            start: 0.5,
            end: 1.0,
        }
        .split([1.0, 4.0]);
        assert_close(x.end, 0.6);
        assert_eq!(y.end, 1.0);
    }

    #[test]
    fn span_split_falls_back_to_equal_parts() {
        for weights in [[0.0, 0.0], [-1.0, 2.0], [f64::NAN, 1.0]] {
            let [a, b] = Span::FULL.split(weights);
            assert_close(a.end, 0.5);
            assert_eq!(b.end, 1.0);
        }
    }

    #[test]
    fn stage_weights_split_rest_after_extract() {
        let [_, rest] = Span::FULL.split([EXTRACT_SHARE, 1.0 - EXTRACT_SHARE]);
        let (generate, concat) = StageWeights::COPY.split(rest);
        assert_close(generate.start, 0.2);
        assert_close(generate.end, 0.3);
        assert_eq!(concat.end, 1.0);

        // 再試行時は送信済みの値から 1 までを再エンコードの比で分け直す
        let (generate, concat) = StageWeights::REENCODE.split(Span {
            start: 0.55,
            end: 1.0,
        });
        assert_close(generate.start, 0.55);
        assert_close(generate.end, 0.8);
        assert_eq!(concat.end, 1.0);
    }

    #[test]
    fn fractions() {
        assert_eq!(fraction(0, 4), 0.0);
        assert_eq!(fraction(1, 4), 0.25);
        assert_eq!(fraction(5, 4), 1.0);
        assert_eq!(fraction(0, 0), 1.0);

        assert_eq!(time_fraction(30.0, 120.0), 0.25);
        assert_eq!(time_fraction(130.0, 120.0), 1.0);
        assert_eq!(time_fraction(-1.0, 120.0), 0.0);
        assert_eq!(time_fraction(1.0, 0.0), 0.0);
        assert_eq!(time_fraction(1.0, f64::NAN), 0.0);
    }

    #[test]
    fn reporter_ignores_backward_and_small_steps() {
        assert_eq!(
            reported(&[0.05, 0.03, 0.055, 0.06, 0.2, 0.1]),
            [0.05, 0.06, 0.2]
        );
    }

    #[test]
    fn reporter_sends_exact_one_percent_steps() {
        // 0.3 + 0.01 は浮動小数点で 0.31 をわずかに超えるが、取りこぼさない
        assert_eq!(reported(&[0.3, 0.31, 0.32]), [0.3, 0.31, 0.32]);
    }

    #[test]
    fn reporter_always_sends_one_once() {
        assert_eq!(reported(&[0.995, 1.0, 1.0]), [0.995, 1.0]);
        // 間引き幅未満の増加でも 1 は送る
        assert_eq!(reported(&[0.999, 1.0]), [0.999, 1.0]);
    }

    #[test]
    fn reporter_clamps_and_skips_nan() {
        assert_eq!(reported(&[-0.5, f64::NAN, 0.5, 3.0]), [0.5, 1.0]);
    }

    #[test]
    fn reporter_current_tracks_sent_value() {
        let mut sink = |_| {};
        let mut reporter = ProgressReporter::new(&mut sink);
        assert_eq!(reporter.current(), 0.0);
        reporter.report(0.42);
        reporter.report(0.425); // 間引かれる
        assert_eq!(reporter.current(), 0.42);
    }
}
