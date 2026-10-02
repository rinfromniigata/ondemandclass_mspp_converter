pub mod plan;
pub mod probe;
pub mod timeline;

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// ffmpeg / ffprobe 1プロセスあたりのタイムアウト
pub const PROCESS_TIMEOUT: Duration = Duration::from_secs(300);

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
