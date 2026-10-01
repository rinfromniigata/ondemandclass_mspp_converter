use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SilentSlideHandling {
    InsertSilence,
    Skip,
}

/// `app.settings.json` の内容
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub ffmpeg_path: PathBuf,
    pub ffprobe_path: PathBuf,
    pub soffice_path: PathBuf,
    #[serde(default = "default_insert_silence")]
    pub silent_slide_handling: SilentSlideHandling,
    #[serde(default = "default_3")]
    pub silent_slide_default_sec: f64,
    #[serde(default = "default_true")]
    pub audio_reencode_on_mismatch: bool,
}

fn default_insert_silence() -> SilentSlideHandling {
    SilentSlideHandling::InsertSilence
}

fn default_3() -> f64 {
    3.0
}

fn default_true() -> bool {
    true
}
