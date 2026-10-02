pub mod concat;
pub mod plan;
pub mod probe;
pub mod timeline;

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// ffmpeg / ffprobe 1プロセスあたりのタイムアウト
pub const PROCESS_TIMEOUT: Duration = Duration::from_secs(300);

/// 結合音声を構成する区間。スライドの表示順に並ぶ。
/// TSの判別共用体（`{ kind: "media", slideIndex, mediaPath }` 等）と同じ形にする
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AudioSegment {
    #[serde(rename_all = "camelCase")]
    Media {
        slide_index: u32,
        /// pptx内のメディアのパート名（例: `ppt/media/media1.m4a`）
        media_path: String,
    },
    #[serde(rename_all = "camelCase")]
    Silence { slide_index: u32, duration_sec: f64 },
}

impl AudioSegment {
    pub fn slide_index(&self) -> u32 {
        match self {
            AudioSegment::Media { slide_index, .. } | AudioSegment::Silence { slide_index, .. } => {
                *slide_index
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConcatResult {
    pub timestamps: Vec<SlideTimestampEntry>,
    /// 再エンコード方式で結合した場合は true
    pub reencoded: bool,
}

/// 結合音声内での1スライド分の区間。`write_timestamps_json` でフロントから受け取るため
/// `Deserialize` も実装する
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlideTimestampEntry {
    /// slideIndex（1始まり、表示順）
    pub slide: u32,
    pub start_sec: f64,
    /// 区間を持たないスライド（skip時の無音スライド）は `start_sec` と同値
    pub end_sec: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_segment_matches_ts_discriminated_union() {
        let json = r#"[
            { "slideIndex": 1, "kind": "media", "mediaPath": "ppt/media/media1.m4a" },
            { "slideIndex": 2, "kind": "silence", "durationSec": 3.5 }
        ]"#;
        let segments: Vec<AudioSegment> = serde_json::from_str(json).unwrap();
        assert_eq!(
            segments,
            vec![
                AudioSegment::Media {
                    slide_index: 1,
                    media_path: "ppt/media/media1.m4a".into(),
                },
                AudioSegment::Silence {
                    slide_index: 2,
                    duration_sec: 3.5,
                },
            ]
        );
        assert_eq!(segments[1].slide_index(), 2);
    }

    #[test]
    fn concat_result_uses_camel_case() {
        let result = ConcatResult {
            timestamps: vec![SlideTimestampEntry {
                slide: 1,
                start_sec: 0.0,
                end_sec: 1.5,
            }],
            reencoded: true,
        };
        assert_eq!(
            serde_json::to_value(&result).unwrap(),
            serde_json::json!({
                "timestamps": [{ "slide": 1, "startSec": 0.0, "endSec": 1.5 }],
                "reencoded": true
            })
        );
    }
}
