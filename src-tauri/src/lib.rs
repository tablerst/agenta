#[cfg(not(test))]
use std::sync::Arc;

pub mod app;
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
    app::init_tracing();
    let runtime = tauri::async_runtime::block_on(app::AppRuntime::bootstrap(
        app::BootstrapOptions::default(),
    ))
    .expect("failed to bootstrap Agenta runtime");
    tauri_app::run(Arc::new(runtime));
}
