use serde_json::json;
use thiserror::Error;

use crate::policy::WriteDecision;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("configuration error: {0}")]
    Config(String),
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
    },
    #[error("storage error: {0}")]
    Storage(String),
    #[error("i/o error: {0}")]
    Io(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl AppError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Config(_) => "invalid_arguments",
            Self::InvalidArguments(_) => "invalid_arguments",
            Self::InvalidAction(_) => "invalid_action",
            Self::NotFound { .. } => "not_found",
            Self::Conflict(_) => "conflict",
            Self::PolicyBlocked { decision, .. } => match decision {
                WriteDecision::Auto => "internal_error",
                WriteDecision::RequireHuman => "requires_human_review",
                WriteDecision::Deny => "policy_blocked",
            },
            Self::Storage(_) | Self::Io(_) | Self::Internal(_) => "internal_error",
        }
    }

    pub fn message(&self) -> String {
        self.to_string()
    }

    pub fn details(&self) -> serde_json::Value {
        match self {
            Self::Config(message)
            | Self::InvalidArguments(message)
            | Self::InvalidAction(message)
            | Self::Conflict(message)
            | Self::Storage(message)
            | Self::Io(message)
            | Self::Internal(message) => json!({ "message": message }),
            Self::NotFound { entity, reference } => {
                json!({ "entity": entity, "reference": reference })
            }
            Self::PolicyBlocked { action, decision } => {
                json!({ "action": action, "decision": decision.as_str() })
            }
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }
}

impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

impl From<sqlx::Error> for AppError {
    fn from(error: sqlx::Error) -> Self {
        Self::Storage(error.to_string())
    }
}

impl From<serde_yaml::Error> for AppError {
    fn from(error: serde_yaml::Error) -> Self {
        Self::Config(error.to_string())
    }
}
