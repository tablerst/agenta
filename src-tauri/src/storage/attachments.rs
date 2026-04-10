use std::path::Path;

use sha2::{Digest, Sha256};
use sqlx::query;
use tokio::fs;
use uuid::Uuid;

use crate::domain::Attachment;
use crate::error::{AppError, AppResult};

use super::mapping::{format_time, map_attachment, sanitize_filename};
use super::{SqliteStore, StoredAttachmentFile};

impl SqliteStore {
    pub async fn insert_attachment(&self, attachment: &Attachment) -> AppResult<()> {
        query(
            r#"
            INSERT INTO attachments (
                attachment_id, task_id, kind, mime, original_filename, original_path,
                storage_path, sha256, size_bytes, summary, created_by, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(attachment.attachment_id.to_string())
        .bind(attachment.task_id.to_string())
        .bind(attachment.kind.to_string())
        .bind(&attachment.mime)
        .bind(&attachment.original_filename)
        .bind(&attachment.original_path)
        .bind(&attachment.storage_path)
        .bind(&attachment.sha256)
        .bind(attachment.size_bytes)
        .bind(&attachment.summary)
        .bind(&attachment.created_by)
        .bind(format_time(attachment.created_at)?)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_attachments(&self, task_id: Uuid) -> AppResult<Vec<Attachment>> {
        let rows = query(
            r#"
            SELECT
                attachment_id, task_id, kind, mime, original_filename, original_path,
                storage_path, sha256, size_bytes, summary, created_by, created_at
            FROM attachments
            WHERE task_id = ?
            ORDER BY created_at DESC, attachment_id DESC
            "#,
        )
        .bind(task_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(map_attachment).collect()
    }

    pub async fn get_attachment_by_ref(&self, reference: &str) -> AppResult<Attachment> {
        let row = query(
            r#"
            SELECT
                attachment_id, task_id, kind, mime, original_filename, original_path,
                storage_path, sha256, size_bytes, summary, created_by, created_at
            FROM attachments
            WHERE attachment_id = ?
            "#,
        )
        .bind(reference)
        .fetch_optional(&self.pool)
        .await?;

        row.map(map_attachment)
            .transpose()?
            .ok_or_else(|| AppError::NotFound {
                entity: "attachment".to_string(),
                reference: reference.to_string(),
            })
    }

    pub async fn persist_attachment_file(
        &self,
        task_id: Uuid,
        attachment_id: Uuid,
        source_path: &Path,
    ) -> AppResult<StoredAttachmentFile> {
        let original_filename = source_path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| {
                AppError::InvalidArguments(
                    "attachment source path must point to a file".to_string(),
                )
            })?
            .to_string();
        let original_path = source_path.canonicalize()?.to_string_lossy().to_string();
        let bytes = fs::read(source_path).await?;
        let sha256 = format!("{:x}", Sha256::digest(&bytes));
        let task_dir = self.attachments_dir.join(task_id.to_string());
        fs::create_dir_all(&task_dir).await?;
        let storage_filename = format!(
            "{}_{}",
            attachment_id,
            sanitize_filename(&original_filename)
        );
        let destination = task_dir.join(&storage_filename);
        fs::write(&destination, &bytes).await?;

        let storage_path = destination
            .strip_prefix(&self.attachments_dir)
            .unwrap_or(&destination)
            .to_string_lossy()
            .replace('\\', "/");
        let mime = mime_guess::from_path(source_path)
            .first_or_octet_stream()
            .to_string();

        Ok(StoredAttachmentFile {
            mime,
            original_filename,
            original_path,
            storage_path,
            sha256,
            size_bytes: bytes.len() as i64,
        })
    }
}
