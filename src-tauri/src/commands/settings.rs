use tauri::{AppHandle, Manager};

use crate::settings::{locate, validate, SettingsStatus};
use crate::state::AppState;

/// 設定ファイルを読み込んで検証し、`ok` なら `AppState` に保持する（`ok` でなければ消す）。
/// 戻り値はスペックどおり `Result` で包まないため、参照の `State<'_>` ではなく
/// 所有型の `AppHandle` から `AppState` を取り出す（参照を受ける async コマンドは `Result` を返す必要がある）
#[tauri::command]
pub async fn load_and_validate_settings(app: AppHandle) -> SettingsStatus {
    let status = match locate::settings_path() {
        Ok(path) => validate::load_and_validate(&path),
        Err(e) => SettingsStatus {
            ok: false,
            settings_path: String::new(),
            settings: None,
            errors: vec![e.to_string()],
        },
    };
    app.state::<AppState>()
        .set_settings(status.settings.clone());
    status
}
