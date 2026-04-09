use std::path::PathBuf;

use tracing_subscriber::EnvFilter;

use crate::app::config::load_runtime_config;
use crate::app::config::RuntimeConfig;
use crate::error::AppResult;
use crate::policy::PolicyEngine;
use crate::service::AgentaService;
use crate::storage::SqliteStore;

#[derive(Clone)]
pub struct AgentaApp {
    pub config: RuntimeConfig,
    pub service: AgentaService,
}

#[derive(Clone, Debug, Default)]
pub struct BootstrapOptions {
    pub config_path: Option<PathBuf>,
}

impl AgentaApp {
    pub async fn bootstrap(options: BootstrapOptions) -> AppResult<Self> {
        let config = load_runtime_config(options.config_path)?;
        let store = SqliteStore::open(
            &config.paths.data_dir,
            &config.paths.database_path,
            &config.paths.attachments_dir,
        )
        .await?;
        let policy = PolicyEngine::new(config.policy.clone());
        let service = AgentaService::new(store, policy);

        Ok(Self { config, service })
    }
}

pub fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .try_init();
}
