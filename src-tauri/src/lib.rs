mod commands;
mod db;
mod error;
mod models;
mod srs;

use commands::AppState;
use std::sync::Mutex;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            let app_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_dir)?;
            let conn = db::open(&app_dir.join("flippety.db"))?;
            app.manage(AppState {
                db: Mutex::new(conn),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::decks::list_decks,
            commands::decks::create_deck,
            commands::decks::rename_deck,
            commands::decks::delete_deck,
            commands::cards::list_cards,
            commands::cards::list_deck_tags,
            commands::cards::create_card,
            commands::cards::update_card,
            commands::cards::delete_card,
            commands::study::get_study_batch,
            commands::study::submit_review,
            commands::study::reset_card_progress,
            commands::import_export::export_deck,
            commands::import_export::import_deck,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
