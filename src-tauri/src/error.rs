use serde_json::json;
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use crate::policy::WriteDecision;

pub type AppResult<T> = Result<T, AppError>;

const MIGRATION_RECOVERY_GUIDANCE: &str = "Back up the database before recovery. Do not edit or delete _sqlx_migrations manually unless following a verified repair plan. Published migration files must remain immutable; schema changes should use a new migration version.";

#[derive(Debug, Error)]
pub enum AppError {
    #[error("configuration error: {0}")]
    Config(String),
    #[error("ambiguous context: {0}")]
    AmbiguousContext(String),
    #[error("invalid arguments: {0}")]
    InvalidArguments(String),
    #[error("invalid action: {0}")]
    InvalidAction(String),
    #[error("resource not found: {entity} {reference}")]
    NotFound { entity: String, reference: String },
    #[error("resource conflict: {0}")]
    Conflict(String),
    #[error("write action blocked by policy: {action}")]
    PolicyBlocked {
        action: String,
        decision: WriteDecision,
        approval_request_id: Option<Uuid>,
        request_summary: Option<String>,
        payload_snapshot: Option<Value>,
    },
    #[error("storage error: {0}")]
    Storage(String),
    #[error("storage is busy: {0}")]
    StorageBusy(String),
    #[error("database migration error: {message}")]
    Migration {
        kind: &'static str,
        version: Option<i64>,
        message: String,
        guidance: &'static str,
    },
    #[error("i/o error: {0}")]
    Io(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl AppError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Config(_) => "invalid_arguments",
            Self::AmbiguousContext(_) => "ambiguous_context",
            Self::InvalidArguments(_) => "invalid_arguments",
            Self::InvalidAction(_) => "invalid_action",
            Self::NotFound { .. } => "not_found",
            Self::Conflict(_) => "conflict",
            Self::PolicyBlocked { decision, .. } => match decision {
                WriteDecision::Auto => "internal_error",
                WriteDecision::RequireHuman => "requires_human_review",
                WriteDecision::Deny => "policy_blocked",
            },
            Self::Migration { .. } => "migration_error",
            Self::StorageBusy(_) => "storage_busy",
            Self::Storage(_) | Self::Io(_) | Self::Internal(_) => "internal_error",
        }
    }

    pub fn message(&self) -> String {
        self.to_string()
    }

    pub fn details(&self) -> serde_json::Value {
        match self {
            Self::Config(message)
            | Self::AmbiguousContext(message)
            | Self::InvalidArguments(message)
            | Self::InvalidAction(message)
            | Self::Conflict(message)
            | Self::Storage(message)
            | Self::Io(message)
            | Self::Internal(message) => json!({ "message": message }),
            Self::Migration {
                kind,
                version,
                message,
                guidance,
            } => json!({
                "message": message,
                "kind": kind,
                "version": version,
                "guidance": guidance,
                "retryable": false,
            }),
            Self::StorageBusy(message) => json!({
                "message": message,
                "retryable": true,
            }),
            Self::NotFound { entity, reference } => {
                json!({ "entity": entity, "reference": reference })
            }
            Self::PolicyBlocked {
                action,
                decision,
                approval_request_id,
                request_summary,
                payload_snapshot,
            } => {
                json!({
                    "action": action,
                    "decision": decision.as_str(),
                    "approval_request_id": approval_request_id.map(|value| value.to_string()),
                    "request_summary": request_summary,
                    "payload_snapshot": payload_snapshot,
                })
            }
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }

    fn migration(kind: &'static str, version: Option<i64>, message: impl Into<String>) -> Self {
        Self::Migration {
            kind,
            version,
            message: message.into(),
            guidance: MIGRATION_RECOVERY_GUIDANCE,
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

impl From<sqlx::Error> for AppError {
    fn from(error: sqlx::Error) -> Self {
        let message = error.to_string();
        if is_sqlite_busy_error(&error) {
            Self::StorageBusy(message)
        } else {
            Self::Storage(message)
        }
    }
}

impl From<sqlx::migrate::MigrateError> for AppError {
    fn from(error: sqlx::migrate::MigrateError) -> Self {
        match &error {
            sqlx::migrate::MigrateError::Execute(source) if is_sqlite_busy_error(source) => {
                Self::StorageBusy(error.to_string())
            }
            sqlx::migrate::MigrateError::ExecuteMigration(source, _)
                if is_sqlite_busy_error(source) =>
            {
                Self::StorageBusy(error.to_string())
            }
            sqlx::migrate::MigrateError::Execute(_) => {
                Self::migration("execute", None, error.to_string())
            }
            sqlx::migrate::MigrateError::ExecuteMigration(_, version) => {
                Self::migration("execute_migration", Some(*version), error.to_string())
            }
            sqlx::migrate::MigrateError::VersionMissing(version) => {
                Self::migration("version_missing", Some(*version), error.to_string())
            }
            sqlx::migrate::MigrateError::VersionMismatch(version) => {
                Self::migration("version_mismatch", Some(*version), error.to_string())
            }
            sqlx::migrate::MigrateError::VersionNotPresent(version) => {
                Self::migration("version_not_present", Some(*version), error.to_string())
            }
            sqlx::migrate::MigrateError::VersionTooOld(version, _) => {
                Self::migration("version_too_old", Some(*version), error.to_string())
            }
            sqlx::migrate::MigrateError::VersionTooNew(version, _) => {
                Self::migration("version_too_new", Some(*version), error.to_string())
            }
            sqlx::migrate::MigrateError::Dirty(version) => {
                Self::migration("dirty", Some(*version), error.to_string())
            }
            sqlx::migrate::MigrateError::Source(_) => {
                Self::migration("source", None, error.to_string())
            }
            sqlx::migrate::MigrateError::ForceNotSupported => {
                Self::migration("unsupported", None, error.to_string())
            }
            _ => Self::migration("unknown", None, error.to_string()),
        }
    }
}

impl From<serde_yaml::Error> for AppError {
    fn from(error: serde_yaml::Error) -> Self {
        Self::Config(error.to_string())
    }
}

fn is_sqlite_busy_error(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::Database(database_error) => {
            if database_error
                .code()
                .and_then(|code| code.parse::<i32>().ok())
                .is_some_and(|code| matches!(code & 0xff, 5 | 6))
            {
                return true;
            }

            is_sqlite_busy_message(database_error.message())
        }
        _ => is_sqlite_busy_message(&error.to_string()),
    }
}

fn is_sqlite_busy_message(message: &str) -> bool {
    let normalized = message.to_ascii_lowercase();
    normalized.contains("database is locked")
        || normalized.contains("database file is locked")
        || normalized.contains("database table is locked")
        || normalized.contains("table in the database is locked")
        || normalized.contains("sqlite_busy")
        || normalized.contains("sqlite_locked")
}
