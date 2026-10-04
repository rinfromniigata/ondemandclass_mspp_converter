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

/// `finish` より前に送る割合の上限（結合を終えても、出力の保存に失敗しうるため 1 にしない）
const BEFORE_FINISH: f64 = 0.99;

/// 取り出し（音声区間の書き出し＋ffprobe）に配分する割合。結合方式は取り出しの後に決まるため、両方式で共通にする。
///
/// 実測（T7-8、release、33区間・約52分の音声）では取り出しが両方式とも約6秒で、全体に占める割合は
/// copy 方式で約8割（全体7秒）、再エンコード方式で約5%（全体125秒）と大きく異なる。
/// 時間のかかる再エンコード方式の表示を優先しつつ、copy 方式で取り出し中に止まって見えないよう中間の値にする
pub const EXTRACT_SHARE: f64 = 0.2;

/// 取り出しの後の残りを「区間生成」と「結合」に分ける比
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StageWeights {
    pub generate: f64,
    pub concat: f64,
}

impl StageWeights {
    /// copy 方式: 区間生成（無音区間の生成）0.1、結合 0.7。
    /// 実測では無音区間なしで 0秒：1.1秒、一部無音で 1.1秒：1.1秒。どちらも短いため初期値のままとする
    pub const COPY: StageWeights = StageWeights {
        generate: 0.1,
        concat: 0.7,
    };
    /// 再エンコード方式: 区間生成（各区間の WAV 正規化＋ffprobe）0.12、結合（AAC エンコード）0.68。
    /// 実測の 16.9秒：101.6秒（約 14：86）に合わせる
    pub const REENCODE: StageWeights = StageWeights {
        generate: 0.12,
        concat: 0.68,
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

/// 音声結合1回分の進捗。「取り出し → 区間生成 → 結合」の各段階の進み具合を全体の割合にして通知する。
///
/// 区間生成と結合の範囲は、結合方式が決まるまで copy 方式の比で仮に置き、
/// `start_mode` で決まった方式の比に置き直す
pub struct ConcatProgress<'a> {
    reporter: ProgressReporter<'a>,
    extract: Span,
    generate: Span,
    concat: Span,
}

impl<'a> ConcatProgress<'a> {
    pub fn new(sink: ProgressSink<'a>) -> Self {
        let [extract, rest] = Span::FULL.split([EXTRACT_SHARE, 1.0 - EXTRACT_SHARE]);
        let (generate, concat) = StageWeights::COPY.split(rest);
        Self {
            reporter: ProgressReporter::new(sink),
            extract,
            generate,
            concat,
        }
    }

    /// 取り出しで `total` 区間のうち `done` 区間を終えた
    pub fn extracted(&mut self, done: usize, total: usize) {
        self.reporter.report(self.extract.at(fraction(done, total)));
    }

    /// 結合方式が決まった。取り出しの後の残りを `weights` の比で区間生成と結合に分ける
    pub fn start_mode(&mut self, weights: StageWeights) {
        let rest = Span {
            start: self.extract.end,
            end: 1.0,
        };
        (self.generate, self.concat) = weights.split(rest);
    }

    /// copy 結合が失敗し、`weights` の方式で区間生成からやり直す。
    /// 割合を戻さないよう、送信済みの値から 1 までを新しい範囲とする
    pub fn restart_with(&mut self, weights: StageWeights) {
        let rest = Span {
            start: self.reporter.current(),
            end: 1.0,
        };
        (self.generate, self.concat) = weights.split(rest);
    }

    /// 区間生成で `total` 区間のうち `done` 区間を終えた
    pub fn generated(&mut self, done: usize, total: usize) {
        self.reporter
            .report(self.generate.at(fraction(done, total)));
    }

    /// 結合で `total_sec` 秒のうち `out_sec` 秒を書き出した。
    /// 1 は出力の保存後（`finish`）にだけ送るため、ここでは `BEFORE_FINISH` で止める
    pub fn concatenating(&mut self, out_sec: f64, total_sec: f64) {
        let ratio = self.concat.at(time_fraction(out_sec, total_sec));
        self.reporter.report(ratio.min(BEFORE_FINISH));
    }

    /// 出力を保存し終えた（1 を通知する）
    pub fn finish(&mut self) {
        self.reporter.report(1.0);
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
        assert_close(generate.end, 0.55 + 0.45 * REENCODE_GENERATE);
        assert_eq!(concat.end, 1.0);
    }

    /// 再エンコード方式で、取り出し後の残りのうち区間生成が占める比（0.12 : 0.68）
    const REENCODE_GENERATE: f64 = 0.12 / 0.8;

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

    /// 送られた割合が 0〜1 に収まり、単調に増える
    fn assert_monotonic(sent: &[f64]) {
        assert!(sent.iter().all(|r| (0.0..=1.0).contains(r)), "{sent:?}");
        assert!(sent.windows(2).all(|w| w[0] < w[1]), "{sent:?}");
    }

    #[test]
    fn concat_progress_copy_mode_runs_through_stages() {
        let mut sent = Vec::new();
        {
            let mut sink = |r| sent.push(r);
            let mut p = ConcatProgress::new(&mut sink);
            p.extracted(1, 2);
            p.extracted(2, 2);
            p.start_mode(StageWeights::COPY);
            p.generated(2, 2);
            p.concatenating(30.0, 60.0);
            p.concatenating(60.0, 60.0);
            p.finish();
        }
        assert_monotonic(&sent);
        // 取り出し 0.2、区間生成 0.1、結合 0.7。結合を終えても 0.99 で止め、1 は finish で送る
        let expected = [0.1, 0.2, 0.3, 0.65, 0.99, 1.0];
        assert_eq!(sent.len(), expected.len(), "{sent:?}");
        for (actual, expected) in sent.iter().zip(expected) {
            assert_close(*actual, expected);
        }
    }

    #[test]
    fn concat_progress_reencode_mode_uses_its_weights() {
        let mut sent = Vec::new();
        {
            let mut sink = |r| sent.push(r);
            let mut p = ConcatProgress::new(&mut sink);
            p.extracted(1, 1);
            p.start_mode(StageWeights::REENCODE);
            p.generated(1, 2);
            p.generated(2, 2);
            p.concatenating(10.0, 10.0);
            p.finish();
        }
        assert_monotonic(&sent);
        // 残り 0.8 を 0.12 : 0.68 で分ける → 区間生成は 0.2〜0.32
        assert_close(sent[0], 0.2);
        assert_close(sent[1], 0.2 + 0.8 * REENCODE_GENERATE / 2.0);
        assert_close(sent[2], 0.32);
        assert_eq!(*sent.last().unwrap(), 1.0);
    }

    #[test]
    fn concat_progress_restart_does_not_go_back() {
        let mut sent = Vec::new();
        {
            let mut sink = |r| sent.push(r);
            let mut p = ConcatProgress::new(&mut sink);
            p.extracted(1, 1);
            p.start_mode(StageWeights::COPY);
            p.generated(1, 1);
            p.concatenating(5.0, 10.0); // copy 結合の途中（0.65）で失敗した
            p.restart_with(StageWeights::REENCODE);
            p.generated(0, 2); // やり直しの最初は起点のまま（送らない）
            p.generated(1, 2);
            p.generated(2, 2);
            p.concatenating(10.0, 10.0);
            p.finish();
        }
        assert_monotonic(&sent);
        assert_close(sent[2], 0.65);
        // 再試行の区間生成は 0.65〜0.65+0.35×0.15
        assert_close(sent[3], 0.65 + 0.35 * REENCODE_GENERATE / 2.0);
        assert_eq!(*sent.last().unwrap(), 1.0);
    }

    #[test]
    fn concat_progress_does_not_send_one_without_finish() {
        let mut sent = Vec::new();
        {
            let mut sink = |r| sent.push(r);
            let mut p = ConcatProgress::new(&mut sink);
            p.extracted(1, 1);
            p.start_mode(StageWeights::COPY);
            p.generated(1, 1);
            // 総尺が不明（0秒）なら結合中は進めない
            p.concatenating(5.0, 0.0);
            // 結合を終えても、保存前（finish 前）は 1 にしない
            p.concatenating(10.0, 10.0);
        }
        assert!(sent.iter().all(|r| *r < 1.0), "{sent:?}");
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
