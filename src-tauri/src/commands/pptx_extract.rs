use std::path::PathBuf;

use crate::pptx::{self, SlideAudioMap};

/// pptx/ppsxを解析し、表示順のスライドごとの音声情報を返す。
/// zipの読み取りとXML解析は同期処理のため、`spawn_blocking` で実行する
#[tauri::command]
pub async fn extract_pptx(input_path: String) -> Result<SlideAudioMap, String> {
    let path = PathBuf::from(input_path);
    tauri::async_runtime::spawn_blocking(move || pptx::extract_from_file(&path))
        .await
        .map_err(|e| format!("pptx解析を実行できませんでした（{e}）"))?
        .map_err(|e| e.to_string())
}
