use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::policy::{PolicyConfig, RawPolicyConfig};

const DEFAULT_DB_FILE: &str = "agenta.sqlite3";
const DEFAULT_MCP_BIND: &str = "127.0.0.1:8787";
const DEFAULT_MCP_PATH: &str = "/mcp";
const DEFAULT_MCP_LOG_FILE: &str = "logs/mcp.jsonl";
const DEFAULT_MCP_UI_BUFFER_LINES: usize = 1000;
const LOCAL_CONFIG_FILE: &str = "agenta.local.yaml";

#[derive(Clone, Debug)]
pub struct AppPaths {
    pub data_dir: PathBuf,
    pub database_path: PathBuf,
    pub attachments_dir: PathBuf,
    pub loaded_config_path: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum McpLogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl Default for McpLogLevel {
    fn default() -> Self {
        Self::Info
    }
}

impl McpLogLevel {
    pub fn allows(self, other: Self) -> bool {
        self.rank() <= other.rank()
    }

    fn rank(self) -> u8 {
        match self {
            Self::Trace => 0,
            Self::Debug => 1,
            Self::Info => 2,
            Self::Warn => 3,
            Self::Error => 4,
        }
    }
}

impl FromStr for McpLogLevel {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "trace" => Ok(Self::Trace),
            "debug" => Ok(Self::Debug),
            "info" => Ok(Self::Info),
            "warn" | "warning" => Ok(Self::Warn),
            "error" => Ok(Self::Error),
            other => Err(format!("unsupported MCP log level: {other}")),
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum McpLogDestination {
    Ui,
    Stdout,
    File,
}

impl FromStr for McpLogDestination {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "ui" => Ok(Self::Ui),
            "stdout" => Ok(Self::Stdout),
            "file" => Ok(Self::File),
            other => Err(format!("unsupported MCP log destination: {other}")),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum McpHostKind {
    Desktop,
    Standalone,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct McpLaunchOverrides {
    pub bind: Option<String>,
    pub path: Option<String>,
    pub autostart: Option<bool>,
    pub log_level: Option<McpLogLevel>,
    pub log_destinations: Option<Vec<McpLogDestination>>,
    pub log_file_path: Option<PathBuf>,
    pub log_ui_buffer_lines: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct McpLogConfig {
    pub level: McpLogLevel,
    pub destinations: Option<Vec<McpLogDestination>>,
    pub file_path: PathBuf,
    pub ui_buffer_lines: usize,
}

#[derive(Clone, Debug)]
pub struct ResolvedMcpLogConfig {
    pub level: McpLogLevel,
    pub destinations: Vec<McpLogDestination>,
    pub file_path: PathBuf,
    pub ui_buffer_lines: usize,
}

#[derive(Clone, Debug)]
pub struct McpConfig {
    pub bind: String,
    pub path: String,
    pub autostart: bool,
    pub log: McpLogConfig,
}

#[derive(Clone, Debug)]
pub struct ResolvedMcpSessionConfig {
    pub bind: String,
    pub path: String,
    pub autostart: bool,
    pub log: ResolvedMcpLogConfig,
}

#[derive(Clone, Debug)]
pub struct RuntimeConfig {
    pub paths: AppPaths,
    pub policy: PolicyConfig,
    pub mcp: McpConfig,
}

impl McpConfig {
    pub fn overlay(&self, overrides: &McpLaunchOverrides) -> Self {
        Self {
            bind: overrides
                .bind
                .clone()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| self.bind.clone()),
            path: overrides
                .path
                .clone()
                .map(|value| normalize_mount_path(&value).unwrap_or_else(|_| self.path.clone()))
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| self.path.clone()),
            autostart: overrides.autostart.unwrap_or(self.autostart),
            log: McpLogConfig {
                level: overrides.log_level.unwrap_or(self.log.level),
                destinations: overrides
                    .log_destinations
                    .clone()
                    .or_else(|| self.log.destinations.clone()),
                file_path: overrides
                    .log_file_path
                    .clone()
                    .unwrap_or_else(|| self.log.file_path.clone()),
                ui_buffer_lines: overrides
                    .log_ui_buffer_lines
                    .filter(|value| *value > 0)
                    .unwrap_or(self.log.ui_buffer_lines),
            },
        }
    }

    pub fn resolve_for_host(&self, host: McpHostKind) -> ResolvedMcpSessionConfig {
        let default_destinations = match host {
            McpHostKind::Desktop => vec![McpLogDestination::Ui, McpLogDestination::File],
            McpHostKind::Standalone => vec![McpLogDestination::Stdout],
        };

        ResolvedMcpSessionConfig {
            bind: self.bind.clone(),
            path: self.path.clone(),
            autostart: self.autostart,
            log: ResolvedMcpLogConfig {
                level: self.log.level,
                destinations: self
                    .log
                    .destinations
                    .clone()
                    .filter(|items| !items.is_empty())
                    .unwrap_or(default_destinations),
                file_path: self.log.file_path.clone(),
                ui_buffer_lines: self.log.ui_buffer_lines.max(1),
            },
        }
    }
}

impl RuntimeConfig {
    pub fn resolve_mcp_session(
        &self,
        host: McpHostKind,
        overrides: &McpLaunchOverrides,
    ) -> AppResult<ResolvedMcpSessionConfig> {
        let effective = self.mcp.overlay(overrides);
        let bind = sanitize_non_empty(&effective.bind, "mcp.bind")?;
        let path = normalize_mount_path(&effective.path)?;
        let file_path = effective.log.file_path.clone();

        Ok(ResolvedMcpSessionConfig {
            bind,
            path,
            autostart: effective.autostart,
            log: ResolvedMcpLogConfig {
                level: effective.log.level,
                destinations: effective
                    .log
                    .destinations
                    .clone()
                    .filter(|items| !items.is_empty())
                    .unwrap_or_else(|| default_log_destinations(host)),
                file_path,
                ui_buffer_lines: effective.log.ui_buffer_lines.max(1),
            },
        })
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct RawConfig {
    paths: Option<RawPaths>,
    policy: Option<RawPolicyConfig>,
    mcp: Option<RawMcpConfig>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct RawPaths {
    data_dir: Option<PathBuf>,
    database_path: Option<PathBuf>,
    attachments_dir: Option<PathBuf>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct RawMcpConfig {
    bind: Option<String>,
    path: Option<String>,
    autostart: Option<bool>,
    log: Option<RawMcpLogConfig>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct RawMcpLogConfig {
    level: Option<McpLogLevel>,
    destinations: Option<Vec<McpLogDestination>>,
    file: Option<RawMcpLogFileConfig>,
    ui: Option<RawMcpLogUiConfig>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct RawMcpLogFileConfig {
    path: Option<PathBuf>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct RawMcpLogUiConfig {
    buffer_lines: Option<usize>,
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

    let raw_mcp = raw_config.mcp.unwrap_or_default();
    let raw_log = raw_mcp.log.unwrap_or_default();

    let bind = sanitize_non_empty(
        &raw_mcp.bind.unwrap_or_else(|| DEFAULT_MCP_BIND.to_string()),
        "mcp.bind",
    )?;
    let path = normalize_mount_path(&raw_mcp.path.unwrap_or_else(|| DEFAULT_MCP_PATH.to_string()))?;
    let log_file_path = raw_log
        .file
        .and_then(|file| file.path)
        .map(|path| resolve_path(&path, &config_base_dir))
        .unwrap_or_else(|| data_dir.join(DEFAULT_MCP_LOG_FILE));

    Ok(RuntimeConfig {
        paths: AppPaths {
            data_dir,
            database_path,
            attachments_dir,
            loaded_config_path: config_path,
        },
        policy: PolicyConfig::from_raw(raw_config.policy.unwrap_or_default()),
        mcp: McpConfig {
            bind,
            path,
            autostart: raw_mcp.autostart.unwrap_or(false),
            log: McpLogConfig {
                level: raw_log.level.unwrap_or_default(),
                destinations: raw_log.destinations,
                file_path: log_file_path,
                ui_buffer_lines: raw_log
                    .ui
                    .and_then(|ui| ui.buffer_lines)
                    .filter(|value| *value > 0)
                    .unwrap_or(DEFAULT_MCP_UI_BUFFER_LINES),
            },
        },
    })
}

pub fn save_mcp_config_defaults(config_path: &Path, mcp: &McpConfig) -> AppResult<()> {
    let mut raw = load_raw_config(config_path)?;
    let config_dir = config_path
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| AppError::Config("config path must have a parent directory".to_string()))?;

    raw.mcp = Some(RawMcpConfig {
        bind: Some(mcp.bind.clone()),
        path: Some(mcp.path.clone()),
        autostart: Some(mcp.autostart),
        log: Some(RawMcpLogConfig {
            level: Some(mcp.log.level),
            destinations: mcp.log.destinations.clone(),
            file: Some(RawMcpLogFileConfig {
                path: Some(path_for_yaml(&mcp.log.file_path, &config_dir)),
            }),
            ui: Some(RawMcpLogUiConfig {
                buffer_lines: Some(mcp.log.ui_buffer_lines),
            }),
        }),
    });

    let serialized = serde_yaml::to_string(&raw)?;
    fs::write(config_path, serialized).map_err(AppError::from)
}

fn discover_config_path(explicit_config_path: Option<PathBuf>) -> AppResult<Option<PathBuf>> {
    if let Some(path) = explicit_config_path {
        return Ok(Some(path));
    }

    if let Ok(path) = env::var("AGENTA_CONFIG") {
        return Ok(Some(PathBuf::from(path)));
    }

    let current_dir = env::current_dir().map_err(AppError::from)?;
    for candidate in local_config_candidates(&current_dir) {
        if candidate.exists() {
            return Ok(Some(candidate));
        }
    }

    Ok(None)
}

fn local_config_candidates(current_dir: &Path) -> Vec<PathBuf> {
    let mut candidates = vec![current_dir.join(LOCAL_CONFIG_FILE)];

    // `bun run tauri dev` commonly launches the desktop binary from `src-tauri/`,
    // while the repo-local runtime config lives at the workspace root.
    if let Some(parent) = current_dir.parent() {
        let parent_candidate = parent.join(LOCAL_CONFIG_FILE);
        if parent_candidate != candidates[0] {
            candidates.push(parent_candidate);
        }
    }

    candidates
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

fn path_for_yaml(path: &Path, base_dir: &Path) -> PathBuf {
    path.strip_prefix(base_dir)
        .map(Path::to_path_buf)
        .unwrap_or_else(|_| path.to_path_buf())
}

fn sanitize_non_empty(value: &str, field: &str) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::Config(format!("{field} must not be empty")));
    }
    Ok(trimmed.to_string())
}

fn normalize_mount_path(value: &str) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::Config("mcp.path must not be empty".to_string()));
    }
    if trimmed == "/" {
        return Ok(DEFAULT_MCP_PATH.to_string());
    }
    Ok(if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    })
}

fn default_log_destinations(host: McpHostKind) -> Vec<McpLogDestination> {
    match host {
        McpHostKind::Desktop => vec![McpLogDestination::Ui, McpLogDestination::File],
        McpHostKind::Standalone => vec![McpLogDestination::Stdout],
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
    use std::path::PathBuf;
    use std::sync::{Mutex, OnceLock};

    use tempfile::tempdir;

    use super::{
        load_runtime_config, save_mcp_config_defaults, McpConfig, McpHostKind, McpLaunchOverrides,
        McpLogConfig, McpLogDestination, McpLogLevel,
    };

    fn environment_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

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

    #[test]
    fn discovers_repo_local_config_from_src_tauri_workdir() {
        let _guard = environment_lock().lock().expect("lock test environment");
        let tempdir = tempdir().expect("tempdir");
        let repo_root = tempdir.path();
        let src_tauri_dir = repo_root.join("src-tauri");
        std::fs::create_dir_all(&src_tauri_dir).expect("create src-tauri directory");

        let config_path = repo_root.join("agenta.local.yaml");
        std::fs::write(
            &config_path,
            "paths:\n  data_dir: ./local-data\nmcp:\n  autostart: true\n",
        )
        .expect("write config");

        let original_dir = std::env::current_dir().expect("read current dir");
        let original_config = std::env::var_os("AGENTA_CONFIG");
        std::env::remove_var("AGENTA_CONFIG");
        std::env::set_current_dir(&src_tauri_dir).expect("switch current dir");

        let result = load_runtime_config(None);

        std::env::set_current_dir(original_dir).expect("restore current dir");
        match original_config {
            Some(value) => std::env::set_var("AGENTA_CONFIG", value),
            None => std::env::remove_var("AGENTA_CONFIG"),
        }

        let config = result.expect("load config from parent workspace");
        assert_eq!(config.paths.loaded_config_path, Some(config_path));
        assert!(config.mcp.autostart);
        assert_eq!(config.paths.data_dir, repo_root.join("local-data"));
    }

    #[test]
    fn applies_mcp_defaults_and_host_destinations() {
        let config = McpConfig {
            bind: "127.0.0.1:8787".to_string(),
            path: "/mcp".to_string(),
            autostart: false,
            log: McpLogConfig {
                level: McpLogLevel::Info,
                destinations: None,
                file_path: PathBuf::from("logs/mcp.jsonl"),
                ui_buffer_lines: 1000,
            },
        };

        let resolved_desktop = config.resolve_for_host(McpHostKind::Desktop);
        assert_eq!(
            resolved_desktop.log.destinations,
            vec![McpLogDestination::Ui, McpLogDestination::File]
        );

        let resolved_standalone = config.resolve_for_host(McpHostKind::Standalone);
        assert_eq!(
            resolved_standalone.log.destinations,
            vec![McpLogDestination::Stdout]
        );

        let overridden = config.overlay(&McpLaunchOverrides {
            bind: Some("127.0.0.1:9999".to_string()),
            autostart: Some(true),
            log_level: Some(McpLogLevel::Debug),
            log_destinations: Some(vec![McpLogDestination::Ui]),
            log_file_path: Some(PathBuf::from("session.jsonl")),
            log_ui_buffer_lines: Some(64),
            path: None,
        });
        assert_eq!(overridden.bind, "127.0.0.1:9999");
        assert!(overridden.autostart);
        assert_eq!(overridden.log.level, McpLogLevel::Debug);
        assert_eq!(
            overridden.log.destinations,
            Some(vec![McpLogDestination::Ui])
        );
        assert_eq!(overridden.log.ui_buffer_lines, 64);
    }

    #[test]
    fn persists_extended_mcp_defaults() {
        let tempdir = tempdir().expect("tempdir");
        let config_path = tempdir.path().join("agenta.local.yaml");
        std::fs::write(&config_path, "paths:\n  data_dir: ./data\n").expect("write config");

        let mcp = McpConfig {
            bind: "127.0.0.1:9898".to_string(),
            path: "/runtime".to_string(),
            autostart: true,
            log: McpLogConfig {
                level: McpLogLevel::Warn,
                destinations: Some(vec![McpLogDestination::Ui, McpLogDestination::File]),
                file_path: tempdir.path().join("logs").join("desktop-mcp.jsonl"),
                ui_buffer_lines: 321,
            },
        };

        save_mcp_config_defaults(&config_path, &mcp).expect("save defaults");
        let loaded = load_runtime_config(Some(config_path)).expect("load config");
        assert_eq!(loaded.mcp.bind, "127.0.0.1:9898");
        assert_eq!(loaded.mcp.path, "/runtime");
        assert!(loaded.mcp.autostart);
        assert_eq!(loaded.mcp.log.level, McpLogLevel::Warn);
        assert_eq!(loaded.mcp.log.ui_buffer_lines, 321);
        assert_eq!(
            loaded.mcp.log.destinations,
            Some(vec![McpLogDestination::Ui, McpLogDestination::File])
        );
        assert_eq!(
            loaded.mcp.log.file_path,
            tempdir.path().join("logs").join("desktop-mcp.jsonl")
        );
    }
}
