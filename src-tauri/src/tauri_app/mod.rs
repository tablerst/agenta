use std::sync::Arc;

use serde::Serialize;
use tauri::State;

use crate::{
    app::AppRuntime,
    interface::response::{ErrorEnvelope, SuccessEnvelope, error, success},
};

#[derive(Debug, Serialize)]
struct DesktopRuntimeStatus {
    data_dir: String,
    database_path: String,
    attachments_dir: String,
    mcp_bind: String,
    mcp_path: String,
    project_count: i64,
    task_count: i64,
}

#[tauri::command]
async fn desktop_status(
    runtime: State<'_, Arc<AppRuntime>>,
) -> Result<SuccessEnvelope, ErrorEnvelope> {
    let counts = runtime
        .service
        .service_overview()
        .await
        .map_err(|app_error| error(&app_error))?;
    success(
        "desktop.status",
        DesktopRuntimeStatus {
            data_dir: runtime.config.paths.data_dir.display().to_string(),
            database_path: runtime.config.paths.database_path.display().to_string(),
            attachments_dir: runtime.config.paths.attachments_dir.display().to_string(),
            mcp_bind: runtime.config.mcp.bind.clone(),
            mcp_path: runtime.config.mcp.path.clone(),
            project_count: counts.project_count,
            task_count: counts.task_count,
        },
        "Loaded desktop runtime status",
    )
    .map_err(|app_error| error(&app_error))
}

pub fn run(runtime: Arc<AppRuntime>) {
    tauri::Builder::default()
        .manage(runtime)
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![desktop_status])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
