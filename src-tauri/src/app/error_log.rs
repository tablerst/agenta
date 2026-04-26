use std::backtrace::Backtrace;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::panic::{self, PanicHookInfo};
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{json, Map, Value};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::build_info::{self, BuildInfo};
use crate::error::AppError;

const REDACTED: &str = "[redacted]";

#[derive(Debug, Serialize)]
pub struct ErrorLogEvent {
    timestamp: String,
    level: &'static str,
    surface: String,
    component: String,
    action: String,
    error_code: Option<String>,
    message: String,
    details: Value,
    build: BuildInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    payload: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    backtrace: Option<String>,
}

impl ErrorLogEvent {
    fn app_error(
        surface: impl Into<String>,
        component: impl Into<String>,
        action: impl Into<String>,
        error: &AppError,
    ) -> Self {
        Self {
            timestamp: now_rfc3339(),
            level: "error",
            surface: surface.into(),
            component: component.into(),
            action: action.into(),
            error_code: Some(error.code().to_string()),
            message: error.message(),
            details: redact_sensitive_value(error.details()),
            build: build_info::get(),
            payload: None,
            location: None,
            backtrace: None,
        }
    }

    fn error_message(
        surface: impl Into<String>,
        component: impl Into<String>,
        action: impl Into<String>,
        error_code: impl Into<String>,
        message: impl Into<String>,
        details: Value,
    ) -> Self {
        Self {
            timestamp: now_rfc3339(),
            level: "error",
            surface: surface.into(),
            component: component.into(),
            action: action.into(),
            error_code: Some(error_code.into()),
            message: message.into(),
            details: redact_sensitive_value(details),
            build: build_info::get(),
            payload: None,
            location: None,
            backtrace: None,
        }
    }

    fn panic(surface: impl Into<String>, info: &PanicHookInfo<'_>) -> Self {
        Self {
            timestamp: now_rfc3339(),
            level: "error",
            surface: surface.into(),
            component: "panic_hook".to_string(),
            action: "panic".to_string(),
            error_code: Some("panic".to_string()),
            message: "process panicked".to_string(),
            details: json!({}),
            build: build_info::get(),
            payload: Some(panic_payload(info)),
            location: info.location().map(|location| {
                format!(
                    "{}:{}:{}",
                    location.file(),
                    location.line(),
                    location.column()
                )
            }),
            backtrace: Some(format!("{:?}", Backtrace::force_capture())),
        }
    }
}

pub fn record_app_error(
    path: &Path,
    surface: impl Into<String>,
    component: impl Into<String>,
    action: impl Into<String>,
    error: &AppError,
) -> io::Result<()> {
    append_event(
        path,
        &ErrorLogEvent::app_error(surface, component, action, error),
    )
}

pub fn record_error_message(
    path: &Path,
    surface: impl Into<String>,
    component: impl Into<String>,
    action: impl Into<String>,
    error_code: impl Into<String>,
    message: impl Into<String>,
    details: Value,
) -> io::Result<()> {
    append_event(
        path,
        &ErrorLogEvent::error_message(surface, component, action, error_code, message, details),
    )
}

pub fn install_panic_hook(path: PathBuf, surface: &'static str) {
    let previous_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = append_event(&path, &ErrorLogEvent::panic(surface, info));
        previous_hook(info);
    }));
}

fn append_event(path: &Path, event: &ErrorLogEvent) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let line = serde_json::to_string(event).map_err(io::Error::other)?;
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(line.as_bytes())?;
    file.write_all(b"\n")
}

fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| OffsetDateTime::now_utc().unix_timestamp().to_string())
}

fn panic_payload(info: &PanicHookInfo<'_>) -> String {
    if let Some(payload) = info.payload().downcast_ref::<&str>() {
        return (*payload).to_string();
    }

    if let Some(payload) = info.payload().downcast_ref::<String>() {
        return payload.clone();
    }

    "<non-string panic payload>".to_string()
}

fn redact_sensitive_value(value: Value) -> Value {
    match value {
        Value::Array(values) => {
            Value::Array(values.into_iter().map(redact_sensitive_value).collect())
        }
        Value::Object(map) => Value::Object(redact_sensitive_map(map)),
        other => other,
    }
}

fn redact_sensitive_map(map: Map<String, Value>) -> Map<String, Value> {
    map.into_iter()
        .map(|(key, value)| {
            let value = if is_sensitive_key(&key) {
                Value::String(REDACTED.to_string())
            } else {
                redact_sensitive_value(value)
            };
            (key, value)
        })
        .collect()
}

fn is_sensitive_key(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase();
    normalized.contains("password")
        || normalized.contains("token")
        || normalized.contains("secret")
        || normalized.contains("api_key")
        || normalized.contains("apikey")
        || normalized == "dsn"
        || normalized.ends_with("_dsn")
        || normalized.contains("authorization")
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tempfile::tempdir;

    use super::{record_app_error, record_error_message, redact_sensitive_value};
    use crate::error::AppError;

    #[test]
    fn writer_creates_parent_directory_and_appends_jsonl() {
        let tempdir = tempdir().expect("tempdir");
        let path = tempdir.path().join("nested").join("error.log");
        let error = AppError::Config("bad config".to_string());

        record_app_error(&path, "cli", "command", "test.failure", &error).expect("write log");

        let content = std::fs::read_to_string(path).expect("read log");
        let line = content.lines().next().expect("jsonl line");
        let event: serde_json::Value = serde_json::from_str(line).expect("parse json");
        assert_eq!(event["surface"], "cli");
        assert_eq!(event["component"], "command");
        assert_eq!(event["action"], "test.failure");
        assert_eq!(event["error_code"], "invalid_arguments");
        assert_eq!(event["message"], "configuration error: bad config");
    }

    #[test]
    fn writer_redacts_sensitive_fields_recursively() {
        let tempdir = tempdir().expect("tempdir");
        let path = tempdir.path().join("error.log");

        record_error_message(
            &path,
            "mcp",
            "startup",
            "test.redaction",
            "internal_error",
            "redaction test",
            json!({
                "api_key": "abc",
                "nested": {
                    "postgres_dsn": "postgres://user:password@host/db",
                    "safe": "value"
                },
                "items": [{ "token": "secret-token" }]
            }),
        )
        .expect("write log");

        let content = std::fs::read_to_string(path).expect("read log");
        let event: serde_json::Value =
            serde_json::from_str(content.lines().next().expect("jsonl line")).expect("parse json");
        assert_eq!(event["details"]["api_key"], "[redacted]");
        assert_eq!(event["details"]["nested"]["postgres_dsn"], "[redacted]");
        assert_eq!(event["details"]["nested"]["safe"], "value");
        assert_eq!(event["details"]["items"][0]["token"], "[redacted]");
    }

    #[test]
    fn redaction_leaves_non_sensitive_values_intact() {
        let value = redact_sensitive_value(json!({
            "message": "contains token in value but not key",
            "count": 3
        }));

        assert_eq!(value["message"], "contains token in value but not key");
        assert_eq!(value["count"], 3);
    }
}
