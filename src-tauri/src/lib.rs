pub mod audio;
pub mod commands;
pub mod error;
pub mod fs_util;
pub mod pdf;
pub mod pptx;
pub mod process;
pub mod settings;
pub mod state;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::settings::load_and_validate_settings,
            commands::pptx_extract::extract_pptx,
            commands::audio_process::run_ffmpeg_concat,
            commands::pdf_convert::run_soffice_convert,
            commands::output_files::check_outputs_exist,
            commands::output_files::write_timestamps_json,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
