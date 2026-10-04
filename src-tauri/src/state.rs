use std::sync::{Mutex, MutexGuard};

use crate::error::AppError;
use crate::settings::AppSettings;

/// Tauri の `State` として共有するアプリ状態
#[derive(Debug, Default)]
pub struct AppState {
    /// 検証に通った設定。未読み込み・検証失敗のときは `None`
    settings: Mutex<Option<AppSettings>>,
}

impl AppState {
    /// 現在の設定の複製。未読み込みなら `SettingsNotLoaded`
    pub fn settings(&self) -> Result<AppSettings, AppError> {
        self.lock().clone().ok_or(AppError::SettingsNotLoaded)
    }

    pub fn set_settings(&self, settings: Option<AppSettings>) {
        *self.lock() = settings;
    }

    fn lock(&self) -> MutexGuard<'_, Option<AppSettings>> {
        // 値の差し替えだけなので、他スレッドのpanicで poison されても中身は壊れていない
        self.settings.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::SilentSlideHandling;

    fn sample() -> AppSettings {
        AppSettings {
            ffmpeg_path: "ffmpeg.exe".into(),
            ffprobe_path: "ffprobe.exe".into(),
            soffice_path: "soffice.exe".into(),
            silent_slide_handling: SilentSlideHandling::InsertSilence,
            silent_slide_default_sec: 3.0,
            audio_reencode_on_mismatch: true,
        }
    }

    #[test]
    fn default_state_has_no_settings() {
        let state = AppState::default();
        assert!(matches!(state.settings(), Err(AppError::SettingsNotLoaded)));
    }

    #[test]
    fn set_and_clear_settings() {
        let state = AppState::default();
        state.set_settings(Some(sample()));
        assert_eq!(state.settings().unwrap(), sample());

        state.set_settings(None);
        assert!(matches!(state.settings(), Err(AppError::SettingsNotLoaded)));
    }
}
