use serde::Serialize;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const GIT_COMMIT: Option<&str> = option_env!("AGENTA_BUILD_GIT_COMMIT");
const GIT_COMMIT_SHORT: Option<&str> = option_env!("AGENTA_BUILD_GIT_COMMIT_SHORT");
const GIT_DESCRIBE: Option<&str> = option_env!("AGENTA_BUILD_GIT_DESCRIBE");
const GIT_DIRTY: Option<&str> = option_env!("AGENTA_BUILD_GIT_DIRTY");

#[derive(Debug, Clone, Serialize)]
pub struct BuildInfo {
    pub version: String,
    pub display_version: String,
    pub git_commit: Option<String>,
    pub git_commit_short: Option<String>,
    pub git_describe: Option<String>,
    pub git_dirty: bool,
}

pub fn get() -> BuildInfo {
    let git_commit_short = optional_env(GIT_COMMIT_SHORT);
    let git_dirty = git_dirty();

    BuildInfo {
        version: VERSION.to_string(),
        display_version: display_version_from(git_commit_short.as_deref(), git_dirty),
        git_commit: optional_env(GIT_COMMIT),
        git_commit_short,
        git_describe: optional_env(GIT_DESCRIBE),
        git_dirty,
    }
}

pub fn cli_version(binary_name: &str) -> String {
    let info = get();
    match info.git_describe {
        Some(git_describe) => {
            format!(
                "{binary_name} {} (git: {git_describe})",
                info.display_version
            )
        }
        None => format!("{binary_name} {}", info.display_version),
    }
}

fn display_version_from(git_commit_short: Option<&str>, git_dirty: bool) -> String {
    match git_commit_short {
        Some(short) if git_dirty => format!("{VERSION}+{short}.dirty"),
        Some(short) => format!("{VERSION}+{short}"),
        None => VERSION.to_string(),
    }
}

fn optional_env(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn git_dirty() -> bool {
    matches!(GIT_DIRTY.map(str::trim), Some("true"))
}
