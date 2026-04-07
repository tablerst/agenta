use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use serde::Deserialize;

use crate::error::{AppError, AppResult};
use crate::policy::{PolicyConfig, RawPolicyConfig};

const DEFAULT_DB_FILE: &str = "agenta.sqlite3";
const DEFAULT_MCP_BIND: &str = "127.0.0.1:8787";
const DEFAULT_MCP_PATH: &str = "/mcp";

#[derive(Clone, Debug)]
pub struct AppPaths {
    pub data_dir: PathBuf,
    pub database_path: PathBuf,
    pub attachments_dir: PathBuf,
    pub loaded_config_path: Option<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct McpConfig {
    pub bind: String,
    pub path: String,
}

#[derive(Clone, Debug)]
pub struct RuntimeConfig {
    pub paths: AppPaths,
    pub policy: PolicyConfig,
    pub mcp: McpConfig,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct RawConfig {
    paths: Option<RawPaths>,
    policy: Option<RawPolicyConfig>,
    mcp: Option<RawMcpConfig>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct RawPaths {
    data_dir: Option<PathBuf>,
    database_path: Option<PathBuf>,
    attachments_dir: Option<PathBuf>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct RawMcpConfig {
    bind: Option<String>,
    path: Option<String>,
}

pub fn load_runtime_config(explicit_config_path: Option<PathBuf>) -> AppResult<RuntimeConfig> {
    let project_dirs = ProjectDirs::from("com", "choriko", "agenta").ok_or_else(|| {
        AppError::Config("failed to determine system application data directory".to_string())
    })?;
    let default_data_dir = project_dirs.data_dir().to_path_buf();

    let config_path = discover_config_path(explicit_config_path)?;
    let raw_config = match config_path.as_ref() {
        Some(path) => load_raw_config(path)?,
        None => RawConfig::default(),
    };

    let config_base_dir = config_path
        .as_ref()
        .and_then(|path| path.parent())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    let data_dir = raw_config
        .paths
        .as_ref()
        .and_then(|paths| paths.data_dir.as_ref())
        .map(|path| resolve_path(path, &config_base_dir))
        .unwrap_or(default_data_dir);
    let database_path = raw_config
        .paths
        .as_ref()
        .and_then(|paths| paths.database_path.as_ref())
        .map(|path| resolve_path(path, &config_base_dir))
        .unwrap_or_else(|| data_dir.join(DEFAULT_DB_FILE));
    let attachments_dir = raw_config
        .paths
        .as_ref()
        .and_then(|paths| paths.attachments_dir.as_ref())
        .map(|path| resolve_path(path, &config_base_dir))
        .unwrap_or_else(|| data_dir.join("attachments"));

    Ok(RuntimeConfig {
        paths: AppPaths {
            data_dir,
            database_path,
            attachments_dir,
            loaded_config_path: config_path,
        },
        policy: PolicyConfig::from_raw(raw_config.policy.unwrap_or_default()),
        mcp: McpConfig {
            bind: raw_config
                .mcp
                .as_ref()
                .and_then(|mcp| mcp.bind.clone())
                .unwrap_or_else(|| DEFAULT_MCP_BIND.to_string()),
            path: raw_config
                .mcp
                .as_ref()
                .and_then(|mcp| mcp.path.clone())
                .unwrap_or_else(|| DEFAULT_MCP_PATH.to_string()),
        },
    })
}

fn discover_config_path(explicit_config_path: Option<PathBuf>) -> AppResult<Option<PathBuf>> {
    if let Some(path) = explicit_config_path {
        return Ok(Some(path));
    }

    if let Ok(path) = env::var("AGENTA_CONFIG") {
        return Ok(Some(PathBuf::from(path)));
    }

    let current_dir = env::current_dir().map_err(AppError::from)?;
    let local = current_dir.join("agenta.local.yaml");
    if local.exists() {
        return Ok(Some(local));
    }

    Ok(None)
}

fn load_raw_config(path: &Path) -> AppResult<RawConfig> {
    let content = fs::read_to_string(path).map_err(AppError::from)?;
    let expanded = expand_env_vars(&content)?;
    Ok(serde_yaml::from_str::<RawConfig>(&expanded)?)
}

fn resolve_path(path: &Path, base_dir: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base_dir.join(path)
    }
}

fn expand_env_vars(content: &str) -> AppResult<String> {
    let chars = content.chars().collect::<Vec<_>>();
    let mut output = String::with_capacity(content.len());
    let mut index = 0;

    while index < chars.len() {
        if chars[index] == '$' && chars.get(index + 1) == Some(&'{') {
            index += 2;
            let start = index;
            while index < chars.len() && chars[index] != '}' {
                index += 1;
            }
            if index >= chars.len() {
                return Err(AppError::Config(
                    "unterminated environment variable placeholder".to_string(),
                ));
            }
            let variable = chars[start..index].iter().collect::<String>();
            let value = env::var(&variable).map_err(|_| {
                AppError::Config(format!(
                    "missing environment variable for config expansion: {variable}"
                ))
            })?;
            output.push_str(&value);
            index += 1;
            continue;
        }

        output.push(chars[index]);
        index += 1;
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::load_runtime_config;

    #[test]
    fn resolves_relative_paths_from_config_directory() {
        let tempdir = tempdir().expect("tempdir");
        let config_path = tempdir.path().join("agenta.local.yaml");
        std::fs::write(
            &config_path,
            "paths:\n  data_dir: data\n  database_path: data/custom.sqlite3\n  attachments_dir: files\n",
        )
        .expect("write config");

        let config = load_runtime_config(Some(config_path)).expect("load config");
        assert_eq!(config.paths.data_dir, tempdir.path().join("data"));
        assert_eq!(
            config.paths.database_path,
            tempdir.path().join("data").join("custom.sqlite3")
        );
        assert_eq!(config.paths.attachments_dir, tempdir.path().join("files"));
    }

    #[test]
    fn expands_environment_variables() {
        let tempdir = tempdir().expect("tempdir");
        let config_path = tempdir.path().join("agenta.local.yaml");
        std::env::set_var("AGENTA_TEST_DATA", "expanded");
        std::fs::write(&config_path, "paths:\n  data_dir: ${AGENTA_TEST_DATA}\n").expect("write");

        let config = load_runtime_config(Some(config_path)).expect("load config");
        assert_eq!(config.paths.data_dir, tempdir.path().join("expanded"));
    }
}
