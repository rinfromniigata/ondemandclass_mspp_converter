//! 結合音声内での各スライドの開始・終了時刻の計算。

use super::SlideTimestampEntry;

/// `slide_indices`（表示順の全スライド）について、結合音声内の区間を求める（純粋関数）。
///
/// `segment_durations` は結合順の（slide_index, 秒）。スライドごとに、最初の区間の開始から
/// 最後の区間の終了までを記録する。区間を持たないスライドは、その時点の累積値で start = end とする。
///
/// 区間が `slide_indices` の順に並んでいない場合（どのスライドにも割り当てられない区間が残る場合）と、
/// 秒が負・有限でない場合はエラーの文言を返す
pub fn build_timeline(
    slide_indices: &[u32],
    segment_durations: &[(u32, f64)],
) -> Result<Vec<SlideTimestampEntry>, String> {
    if let Some((slide, sec)) = segment_durations
        .iter()
        .find(|(_, sec)| !sec.is_finite() || *sec < 0.0)
    {
        return Err(format!(
            "スライド{slide}の音声区間の長さが不正です（{sec}秒）"
        ));
    }

    let mut segments = segment_durations.iter().peekable();
    let mut elapsed = 0.0;
    let mut entries = Vec::with_capacity(slide_indices.len());
    for &slide in slide_indices {
        let start_sec = elapsed;
        while let Some((_, sec)) = segments.next_if(|(seg_slide, _)| *seg_slide == slide) {
            elapsed += sec;
        }
        entries.push(SlideTimestampEntry {
            slide,
            start_sec,
            end_sec: elapsed,
        });
    }

    match segments.next() {
        None => Ok(entries),
        Some((slide, _)) => Err(format!(
            "スライド{slide}の音声区間が、スライドの表示順に並んでいません"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(slide: u32, start_sec: f64, end_sec: f64) -> SlideTimestampEntry {
        SlideTimestampEntry {
            slide,
            start_sec,
            end_sec,
        }
    }

    #[test]
    fn accumulates_one_segment_per_slide() {
        let timeline = build_timeline(&[1, 2, 3], &[(1, 42.5), (2, 45.75), (3, 10.0)]).unwrap();
        assert_eq!(
            timeline,
            vec![
                entry(1, 0.0, 42.5),
                entry(2, 42.5, 88.25),
                entry(3, 88.25, 98.25),
            ]
        );
    }

    #[test]
    fn slide_with_multiple_segments_spans_all_of_them() {
        let timeline = build_timeline(&[1, 2], &[(1, 1.5), (1, 2.5), (1, 1.0), (2, 3.0)]).unwrap();
        assert_eq!(timeline, vec![entry(1, 0.0, 5.0), entry(2, 5.0, 8.0)]);
    }

    #[test]
    fn skipped_silent_slide_has_zero_length_at_current_position() {
        // skip 時：スライド2・3は区間を持たない
        let timeline = build_timeline(&[1, 2, 3, 4], &[(1, 4.0), (4, 6.0)]).unwrap();
        assert_eq!(
            timeline,
            vec![
                entry(1, 0.0, 4.0),
                entry(2, 4.0, 4.0),
                entry(3, 4.0, 4.0),
                entry(4, 4.0, 10.0),
            ]
        );
    }

    #[test]
    fn leading_silence_is_counted() {
        // insert_silence 時：先頭スライドが無音区間
        let timeline = build_timeline(&[1, 2], &[(1, 3.0), (2, 7.25)]).unwrap();
        assert_eq!(timeline, vec![entry(1, 0.0, 3.0), entry(2, 3.0, 10.25)]);
    }

    #[test]
    fn leading_skipped_slides_start_at_zero() {
        // skip 時：先頭のスライドが無音で区間を持たない
        let timeline = build_timeline(&[1, 2, 3], &[(3, 5.0)]).unwrap();
        assert_eq!(
            timeline,
            vec![entry(1, 0.0, 0.0), entry(2, 0.0, 0.0), entry(3, 0.0, 5.0)]
        );
    }

    #[test]
    fn trailing_skipped_slide_ends_at_total() {
        let timeline = build_timeline(&[1, 2], &[(1, 2.0)]).unwrap();
        assert_eq!(timeline, vec![entry(1, 0.0, 2.0), entry(2, 2.0, 2.0)]);
    }

    #[test]
    fn display_order_is_taken_from_slide_indices() {
        let timeline = build_timeline(&[1, 2], &[(1, 1.0), (2, 2.0)]).unwrap();
        assert_eq!(timeline.iter().map(|e| e.slide).collect::<Vec<_>>(), [1, 2]);
        assert!(build_timeline(&[], &[]).unwrap().is_empty());
    }

    #[test]
    fn errors_when_segments_are_out_of_order() {
        let err = build_timeline(&[1, 2], &[(2, 1.0), (1, 1.0)]).unwrap_err();
        assert_eq!(
            err,
            "スライド1の音声区間が、スライドの表示順に並んでいません"
        );
    }

    #[test]
    fn errors_on_segment_of_unknown_slide() {
        let err = build_timeline(&[1, 2], &[(1, 1.0), (5, 1.0)]).unwrap_err();
        assert_eq!(
            err,
            "スライド5の音声区間が、スライドの表示順に並んでいません"
        );
    }

    #[test]
    fn errors_on_invalid_duration() {
        let err = build_timeline(&[1], &[(1, -0.5)]).unwrap_err();
        assert_eq!(err, "スライド1の音声区間の長さが不正です（-0.5秒）");
        assert!(build_timeline(&[1], &[(1, f64::NAN)]).is_err());
        assert!(build_timeline(&[1], &[(1, f64::INFINITY)]).is_err());
    }
}
