use sqlx::{QueryBuilder, Sqlite, query};
use uuid::Uuid;

use crate::domain::{ApprovalRequest, ApprovalStatus};
use crate::error::{AppError, AppResult};

use super::mapping::{format_time, map_approval_request};
use super::SqliteStore;

impl SqliteStore {
    pub async fn insert_approval_request(&self, request: &ApprovalRequest) -> AppResult<()> {
        query(
            r#"
            INSERT INTO approval_requests (
                request_id,
                action,
                requested_via,
                resource_ref,
                payload_json,
                request_summary,
                requested_at,
                requested_by,
                reviewed_at,
                reviewed_by,
                review_note,
                result_json,
                error_json,
                status
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(request.request_id.to_string())
        .bind(&request.action)
        .bind(request.requested_via.to_string())
        .bind(&request.resource_ref)
        .bind(request.payload_json.to_string())
        .bind(&request.request_summary)
        .bind(format_time(request.requested_at)?)
        .bind(&request.requested_by)
        .bind(request.reviewed_at.map(format_time).transpose()?)
        .bind(&request.reviewed_by)
        .bind(&request.review_note)
        .bind(request.result_json.as_ref().map(serde_json::Value::to_string))
        .bind(request.error_json.as_ref().map(serde_json::Value::to_string))
        .bind(request.status.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_approval_request(&self, request_id: Uuid) -> AppResult<ApprovalRequest> {
        let row = query(
            r#"
            SELECT
                request_id, action, requested_via, resource_ref, payload_json,
                request_summary, requested_at, requested_by, reviewed_at,
                reviewed_by, review_note, result_json, error_json, status
            FROM approval_requests
            WHERE request_id = ?
            "#,
        )
        .bind(request_id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        row.map(map_approval_request)
            .transpose()?
            .ok_or_else(|| AppError::NotFound {
                entity: "approval_request".to_string(),
                reference: request_id.to_string(),
            })
    }

    pub async fn list_approval_requests(
        &self,
        status: Option<ApprovalStatus>,
    ) -> AppResult<Vec<ApprovalRequest>> {
        let mut builder = QueryBuilder::<Sqlite>::new(
            r#"
            SELECT
                request_id, action, requested_via, resource_ref, payload_json,
                request_summary, requested_at, requested_by, reviewed_at,
                reviewed_by, review_note, result_json, error_json, status
            FROM approval_requests
            WHERE 1 = 1
            "#,
        );
        if let Some(status) = status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }
        builder.push(" ORDER BY requested_at DESC");

        let rows = builder.build().fetch_all(&self.pool).await?;
        rows.into_iter().map(map_approval_request).collect()
    }

    pub async fn update_approval_request(&self, request: &ApprovalRequest) -> AppResult<()> {
        query(
            r#"
            UPDATE approval_requests
            SET
                reviewed_at = ?,
                reviewed_by = ?,
                review_note = ?,
                result_json = ?,
                error_json = ?,
                status = ?
            WHERE request_id = ?
            "#,
        )
        .bind(request.reviewed_at.map(format_time).transpose()?)
        .bind(&request.reviewed_by)
        .bind(&request.review_note)
        .bind(request.result_json.as_ref().map(serde_json::Value::to_string))
        .bind(request.error_json.as_ref().map(serde_json::Value::to_string))
        .bind(request.status.to_string())
        .bind(request.request_id.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
