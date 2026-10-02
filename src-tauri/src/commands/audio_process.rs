use std::path::PathBuf;

use tauri::State;

use crate::audio::concat::{concat_audio, FfmpegTools};
use crate::audio::{AudioSegment, ConcatResult};
use crate::state::AppState;

/// `segments` の音声と無音を結合して `out_path` に m4a で書き出し、各スライドの区間を返す。
/// ffmpeg / ffprobe のパスは `AppState` の設定から取る。外部プロセスを待つため `spawn_blocking` で実行する
#[tauri::command]
pub async fn run_ffmpeg_concat(
    state: State<'_, AppState>,
    input_path: String,
    slide_indices: Vec<u32>,
    segments: Vec<AudioSegment>,
    out_path: String,
    reencode_on_mismatch: bool,
) -> Result<ConcatResult, String> {
    let settings = state.settings().map_err(|e| e.to_string())?;
    let input = PathBuf::from(input_path);
    let out = PathBuf::from(out_path);
    tauri::async_runtime::spawn_blocking(move || {
        let tools = FfmpegTools {
            ffmpeg: &settings.ffmpeg_path,
            ffprobe: &settings.ffprobe_path,
        };
        concat_audio(
            &tools,
            &input,
            &slide_indices,
            &segments,
            &out,
            reencode_on_mismatch,
        )
    })
    .await
    .map_err(|e| format!("音声結合を実行できませんでした（{e}）"))?
    .map_err(|e| e.to_string())
}
