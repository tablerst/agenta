#[cfg(not(test))]
use std::sync::Arc;

pub mod app;
pub mod build_info;
pub mod domain;
pub mod error;
pub mod interface;
pub mod policy;
pub mod search;
pub mod service;
pub mod storage;
pub mod sync;
// The Desktop shell pulls in Windows GUI/runtime dependencies that make the
// default lib test harness fail to start on this environment. Keep it out of
// `cargo test --lib` and cover shell behavior through non-lib test targets.
#[cfg(not(test))]
pub mod tauri_app;

#[cfg(not(test))]
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let error_log_path = app::resolve_error_log_path(None);
    app::install_panic_hook(error_log_path.clone(), "desktop");
    app::init_tracing();
    let runtime = match tauri::async_runtime::block_on(app::AppRuntime::bootstrap(
        app::BootstrapOptions::default(),
    )) {
        Ok(runtime) => runtime,
        Err(error) => {
            let _ = app::record_app_error(
                &error_log_path,
                "desktop",
                "bootstrap",
                "desktop.bootstrap",
                &error,
            );
            return;
        }
    };
    tauri_app::run(Arc::new(runtime));
}
