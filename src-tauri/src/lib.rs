mod db;
mod directories;
mod reminders;

use std::fs;
use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let portable_dir = std::env::current_exe()
                .ok()
                .and_then(|exe| exe.parent().map(|parent| parent.to_path_buf()))
                .filter(|dir| dir.join("portable.mode").is_file())
                .map(|dir| dir.join("data"));
            let app_dir = match portable_dir {
                Some(dir) => dir,
                None => app.path().app_data_dir()?,
            };
            let backup_dir = app_dir.join("backups");
            fs::create_dir_all(&backup_dir)?;
            let db_path = app_dir.join("qingdan.sqlite3");
            db::prepare_upgrade(&db_path, &backup_dir).map_err(std::io::Error::other)?;
            db::initialize(&db_path).map_err(std::io::Error::other)?;
            if db::daily_backup_enabled(&db_path).map_err(std::io::Error::other)? {
                db::create_daily_backup_if_needed(&db_path, &backup_dir)
                    .map_err(std::io::Error::other)?;
            }
            app.manage(db::AppState { db_path, backup_dir, operations: std::sync::Mutex::new(()) });
            reminders::start(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            db::task_list,
            db::task_create,
            db::task_update,
            db::task_complete,
            db::task_delete,
            db::task_delete_many,
            directories::directory_pick,
            directories::directory_open,
            reminders::email_status,
            reminders::email_auth_save,
            reminders::email_auth_clear,
            reminders::email_test,
            reminders::email_retry_failed,
            directories::tasks_export_save,
            db::task_restore,
            db::stats_get,
            db::category_list,
            db::category_save,
            db::category_delete,
            db::settings_get,
            db::settings_save,
            db::backup_create,
            db::backup_list,
            db::backup_restore,
            db::demo_reset
        ])
        .run(tauri::generate_context!())
        .expect("启动清单应用失败");
}

