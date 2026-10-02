pub mod commands;
pub mod error;
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
        // 各Phaseで実装したコマンドをここへ追加する
        .invoke_handler(tauri::generate_handler![
            commands::settings::load_and_validate_settings,
            commands::pptx_extract::extract_pptx,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
