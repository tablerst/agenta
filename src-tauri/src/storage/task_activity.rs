use super::*;

impl SqliteStore {
    pub async fn insert_activity(&self, activity: &TaskActivity) -> AppResult<()> {
        query(
            r#"
            INSERT INTO task_activities (
                activity_id, task_id, kind, content, activity_search_summary, activity_search_text, created_by, created_at, metadata_json
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(activity.activity_id.to_string())
        .bind(activity.task_id.to_string())
        .bind(activity.kind.to_string())
        .bind(&activity.content)
        .bind(&activity.activity_search_summary)
        .bind(&activity.activity_search_text)
        .bind(&activity.created_by)
        .bind(format_time(activity.created_at)?)
        .bind(activity.metadata_json.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn insert_activity_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        activity: &TaskActivity,
    ) -> AppResult<()> {
        query(
            r#"
            INSERT INTO task_activities (
                activity_id,
                task_id,
                kind,
                content,
                activity_search_summary,
                activity_search_text,
                created_by,
                created_at,
                metadata_json
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(activity.activity_id.to_string())
        .bind(activity.task_id.to_string())
        .bind(activity.kind.to_string())
        .bind(&activity.content)
        .bind(&activity.activity_search_summary)
        .bind(&activity.activity_search_text)
        .bind(&activity.created_by)
        .bind(format_time(activity.created_at)?)
        .bind(activity.metadata_json.to_string())
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    pub async fn replace_activity_chunks_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        activity: &TaskActivity,
        chunk_source_text: &str,
    ) -> AppResult<()> {
        query("DELETE FROM task_activity_chunks WHERE activity_id = ?")
            .bind(activity.activity_id.to_string())
            .execute(&mut **tx)
            .await?;

        let chunks = build_activity_search_chunks(chunk_source_text);
        let fallback_chunk = if chunks.is_empty() {
            activity.activity_search_summary.clone()
        } else {
            String::new()
        };
        let chunk_texts = if chunks.is_empty() {
            vec![fallback_chunk]
        } else {
            chunks
        };

        for (index, chunk_text) in chunk_texts.into_iter().enumerate() {
            query(
                r#"
                INSERT INTO task_activity_chunks (
                    chunk_id,
                    activity_id,
                    task_id,
                    chunk_index,
                    chunk_text
                ) VALUES (?, ?, ?, ?, ?)
                "#,
            )
            .bind(format!("{}:{index}", activity.activity_id))
            .bind(activity.activity_id.to_string())
            .bind(activity.task_id.to_string())
            .bind(index as i64)
            .bind(chunk_text)
            .execute(&mut **tx)
            .await?;
        }
        Ok(())
    }

    pub async fn rebuild_activity_chunk_index(&self) -> AppResult<()> {
        let activity_count = query("SELECT COUNT(*) AS count FROM task_activities")
            .fetch_one(&self.pool)
            .await?
            .get::<i64, _>("count");
        let chunked_activity_count =
            query("SELECT COUNT(DISTINCT activity_id) AS count FROM task_activity_chunks")
                .fetch_one(&self.pool)
                .await?
                .get::<i64, _>("count");
        if activity_count == 0 || activity_count == chunked_activity_count {
            return Ok(());
        }

        let rows = query(
            r#"
            SELECT activity_id, task_id, kind, content, activity_search_summary, activity_search_text, created_by, created_at, metadata_json
            FROM task_activities
            ORDER BY created_at ASC, activity_id ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        let activities = rows
            .into_iter()
            .map(map_activity)
            .collect::<AppResult<Vec<_>>>()?;

        let mut tx = self.pool.begin().await?;
        query("DELETE FROM task_activity_chunks")
            .execute(&mut *tx)
            .await?;
        for activity in &activities {
            let chunk_source_text = self.activity_chunk_source_text(activity).await?;
            self.replace_activity_chunks_tx(&mut tx, activity, &chunk_source_text)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub(super) async fn activity_chunk_source_text(
        &self,
        activity: &TaskActivity,
    ) -> AppResult<String> {
        if activity.kind != TaskActivityKind::AttachmentRef {
            return Ok(activity.content.clone());
        }

        let storage_path = activity
            .metadata_json
            .get("storage_path")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let Some(storage_path) = storage_path else {
            return Ok(activity.content.clone());
        };

        let attachment_path = self.attachments_dir.join(storage_path);
        let bytes = match fs::read(&attachment_path).await {
            Ok(bytes) => bytes,
            Err(_) => return Ok(activity.content.clone()),
        };
        let mime = mime_guess::from_path(&attachment_path)
            .first_or_octet_stream()
            .to_string();
        let original_filename = attachment_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("attachment");
        let extracted = self.extract_attachment_search_text(&bytes, &mime, original_filename);
        Ok(extracted
            .map(|text| format!("{}\n{text}", activity.content))
            .unwrap_or_else(|| activity.content.clone()))
    }

    pub async fn list_task_activities(&self, task_id: Uuid) -> AppResult<Vec<TaskActivity>> {
        let rows = query(
            r#"
            SELECT activity_id, task_id, kind, content, activity_search_summary, activity_search_text, created_by, created_at, metadata_json
            FROM task_activities
            WHERE task_id = ?
            ORDER BY created_at DESC, activity_id DESC
            "#,
        )
        .bind(task_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(map_activity).collect()
    }
}
