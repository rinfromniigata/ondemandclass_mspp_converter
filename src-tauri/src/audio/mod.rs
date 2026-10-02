pub mod probe;

use std::time::Duration;

/// ffmpeg / ffprobe 1プロセスあたりのタイムアウト
pub const PROCESS_TIMEOUT: Duration = Duration::from_secs(300);
