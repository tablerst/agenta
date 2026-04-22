use std::path::Path;

use sha2::{Digest, Sha256};
use sqlx::{query, Sqlite, Transaction};
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

    pub async fn insert_attachment_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        attachment: &Attachment,
    ) -> AppResult<()> {
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
        .execute(&mut **tx)
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
        let extracted_search_text =
            self.extract_attachment_search_text(&bytes, &mime, &original_filename);

        Ok(StoredAttachmentFile {
            mime,
            original_filename,
            original_path,
            storage_path,
            sha256,
            size_bytes: bytes.len() as i64,
            extracted_search_text,
        })
    }

    pub fn extract_attachment_search_text(
        &self,
        bytes: &[u8],
        mime: &str,
        original_filename: &str,
    ) -> Option<String> {
        extract_attachment_search_text(bytes, mime, original_filename)
    }
}

const MAX_ATTACHMENT_TEXT_BYTES: usize = 256 * 1024;
const MAX_ATTACHMENT_SEARCH_TEXT_CHARS: usize = 12_000;

fn extract_attachment_search_text(
    bytes: &[u8],
    mime: &str,
    original_filename: &str,
) -> Option<String> {
    if bytes.is_empty() || !is_probably_text_attachment(mime, original_filename) {
        return None;
    }

    let sample = &bytes[..bytes.len().min(MAX_ATTACHMENT_TEXT_BYTES)];
    let binary_signals = sample
        .iter()
        .filter(|byte| matches!(byte, 0x00..=0x08 | 0x0B | 0x0C | 0x0E..=0x1A | 0x1C..=0x1F))
        .count();
    if binary_signals > sample.len() / 20 {
        return None;
    }

    let decoded = String::from_utf8_lossy(sample);
    let normalized = decoded
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if normalized.is_empty() {
        return None;
    }

    Some(sample_attachment_text(
        &normalized,
        MAX_ATTACHMENT_SEARCH_TEXT_CHARS,
    ))
}

fn is_probably_text_attachment(mime: &str, original_filename: &str) -> bool {
    if mime.starts_with("text/") {
        return true;
    }
    let lower_mime = mime.to_ascii_lowercase();
    if [
        "json",
        "xml",
        "yaml",
        "javascript",
        "typescript",
        "x-sh",
        "sql",
        "csv",
    ]
    .iter()
    .any(|needle| lower_mime.contains(needle))
    {
        return true;
    }

    let extension = Path::new(original_filename)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());
    matches!(
        extension.as_deref(),
        Some(
            "txt"
                | "md"
                | "markdown"
                | "log"
                | "rst"
                | "json"
                | "yaml"
                | "yml"
                | "toml"
                | "ini"
                | "cfg"
                | "conf"
                | "csv"
                | "tsv"
                | "xml"
                | "html"
                | "htm"
                | "css"
                | "js"
                | "ts"
                | "tsx"
                | "jsx"
                | "rs"
                | "py"
                | "java"
                | "kt"
                | "go"
                | "sql"
                | "sh"
                | "ps1"
                | "bat"
                | "cmd"
                | "vue"
        )
    )
}

fn sample_attachment_text(value: &str, limit: usize) -> String {
    let chars = value.chars().collect::<Vec<_>>();
    if chars.len() <= limit {
        return value.to_string();
    }

    let window = (limit / 3).max(512);
    let start = chars.iter().take(window).collect::<String>();
    let middle_start = chars.len().saturating_sub(window) / 2;
    let middle = chars
        .iter()
        .skip(middle_start)
        .take(window)
        .collect::<String>();
    let end = chars
        .iter()
        .rev()
        .take(window)
        .copied()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();

    format!("{start}\n...\n{middle}\n...\n{end}")
}
