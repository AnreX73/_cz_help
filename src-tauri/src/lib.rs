mod commands;
mod db;
mod documents;
mod models;
mod tax;

use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Manager;

pub struct AppState {
    pub db: Mutex<rusqlite::Connection>,
    pub data_dir: PathBuf,
    pub attachments_dir: PathBuf,
    pub docs_dir: PathBuf,
    pub db_path: PathBuf,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| PathBuf::from("."));
            std::fs::create_dir_all(&data_dir)?;
            let attachments_dir = data_dir.join("attachments");
            std::fs::create_dir_all(&attachments_dir)?;
            let db_path = data_dir.join("cz_help.db");
            let conn = db::open(&db_path).map_err(|e| format!("Не удалось открыть базу: {e}"))?;

            let docs_dir = app
                .path()
                .document_dir()
                .map(|d| d.join("Чеки"))
                .unwrap_or_else(|_| data_dir.join("Чеки"));

            app.manage(AppState {
                db: Mutex::new(conn),
                data_dir,
                attachments_dir,
                docs_dir,
                db_path,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::app_paths,
            commands::list_sellers,
            commands::active_seller,
            commands::save_seller,
            commands::switch_seller,
            commands::set_seller_archived,
            commands::delete_seller,
            commands::get_dashboard,
            commands::list_months,
            commands::set_paid,
            commands::list_entries,
            commands::save_entry,
            commands::delete_entry,
            commands::list_clients,
            commands::save_client,
            commands::set_client_archived,
            commands::delete_client,
            commands::list_services,
            commands::save_service,
            commands::delete_service,
            commands::list_service_categories,
            commands::save_service_category,
            commands::delete_service_category,
            commands::import_receipt,
            commands::receipt_data_url,
            commands::receipt_thumb_data_url,
            commands::preview_document_pdf,
            commands::save_document,
            commands::list_documents,
            commands::get_document,
            commands::document_pdf,
            commands::delete_document,
            commands::export_csv,
            commands::backup,
            commands::open_path,
            commands::reveal_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
