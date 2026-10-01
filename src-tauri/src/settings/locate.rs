use std::path::{Path, PathBuf};

use crate::error::AppError;

pub const SETTINGS_FILE_NAME: &str = "app.settings.json";

/// 設定ファイルのパス（スペック5.2「配置場所」）
/// - debugビルド：リポジトリ直下（`src-tauri` の親）
/// - releaseビルド：実行ファイルと同じフォルダ
pub fn settings_path() -> Result<PathBuf, AppError> {
    Ok(base_dir()?.join(SETTINGS_FILE_NAME))
}

fn base_dir() -> Result<PathBuf, AppError> {
    if cfg!(debug_assertions) {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        manifest_dir.parent().map(Path::to_path_buf).ok_or_else(|| {
            AppError::Message(format!(
                "{} の親フォルダを取得できません",
                manifest_dir.display()
            ))
        })
    } else {
        let exe = std::env::current_exe().map_err(|source| AppError::Io {
            context: "実行ファイルの場所を取得できません".into(),
            source,
        })?;
        exe.parent().map(Path::to_path_buf).ok_or_else(|| {
            AppError::Message(format!("{} の親フォルダを取得できません", exe.display()))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_build_points_to_repository_root() {
        let path = settings_path().unwrap();
        assert_eq!(path.file_name().unwrap(), SETTINGS_FILE_NAME);
        // リポジトリ直下にはひな形が置かれている
        assert!(path
            .parent()
            .unwrap()
            .join("app.settings.example.json")
            .is_file());
    }
}
