use std::path::PathBuf;

use tauri::{AppHandle, Manager, State};

use crate::pdf::convert_to_pdf;
use crate::state::AppState;

/// LibreOffice のアプリ専用プロファイルを置く、アプリのローカルデータフォルダ内のフォルダ名
const PROFILE_DIR_NAME: &str = "lo_profile";

/// pptx/ppsx を PDF に変換して `out_path` に書き出し、そのフルパスを返す。
/// soffice のパスは `AppState` の設定から取る。外部プロセスを待つため `spawn_blocking` で実行する
#[tauri::command]
pub async fn run_soffice_convert(
    app: AppHandle,
    state: State<'_, AppState>,
    input_path: String,
    out_path: String,
) -> Result<String, String> {
    let settings = state.settings().map_err(|e| e.to_string())?;
    let profile_dir = app
        .path()
        .app_local_data_dir()
        .map_err(|e| format!("アプリのデータフォルダを取得できません（{e}）"))?
        .join(PROFILE_DIR_NAME);
    let input = PathBuf::from(input_path);
    let out = PathBuf::from(out_path);
    tauri::async_runtime::spawn_blocking(move || {
        convert_to_pdf(&settings.soffice_path, &profile_dir, &input, &out)
    })
    .await
    .map_err(|e| format!("PDF変換を実行できませんでした（{e}）"))?
    .map(|path| path.display().to_string())
    .map_err(|e| e.to_string())
}
