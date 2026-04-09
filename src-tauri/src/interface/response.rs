use rmcp::ErrorData;
use schemars::{json_schema, JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{AppError, AppResult};

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct SuccessEnvelope {
    pub ok: bool,
    pub action: String,
    #[schemars(schema_with = "any_json_schema")]
    pub result: Value,
    pub summary: String,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ErrorEnvelope {
    pub ok: bool,
    pub error: ErrorBody,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
    pub details: Value,
}

pub fn success(
    action: impl Into<String>,
    result: impl Serialize,
    summary: impl Into<String>,
) -> AppResult<SuccessEnvelope> {
    Ok(SuccessEnvelope {
        ok: true,
        action: action.into(),
        result: serde_json::to_value(result)
            .map_err(|error| AppError::internal(format!("failed to serialize result: {error}")))?,
        summary: summary.into(),
        warnings: Vec::new(),
    })
}

pub fn error(error: &AppError) -> ErrorEnvelope {
    ErrorEnvelope {
        ok: false,
        error: ErrorBody {
            code: error.code().to_string(),
            message: error.message(),
            details: error.details(),
        },
    }
}

pub fn error_to_rmcp(error: AppError) -> ErrorData {
    let data = Some(error.details());
    match error.code() {
        "invalid_arguments" => ErrorData::invalid_params(error.message(), data),
        "not_found" => ErrorData::resource_not_found(error.message(), data),
        _ => ErrorData::internal_error(error.message(), data),
    }
}

fn any_json_schema(_: &mut SchemaGenerator) -> Schema {
    json_schema!({})
}

#[cfg(test)]
mod tests {
    use schemars::schema_for;

    use super::SuccessEnvelope;

    #[test]
    fn success_envelope_result_schema_uses_object_schema_for_any_json() {
        let schema = serde_json::to_value(schema_for!(SuccessEnvelope)).expect("serialize schema");
        let result_schema = &schema["properties"]["result"];

        assert!(
            result_schema.is_object(),
            "result schema should be an object"
        );
        assert!(
            result_schema
                .as_object()
                .expect("result schema object")
                .is_empty(),
            "result schema should stay permissive for arbitrary JSON payloads"
        );
    }
}
