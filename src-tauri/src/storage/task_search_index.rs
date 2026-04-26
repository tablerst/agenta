use super::*;

impl SqliteStore {
    pub async fn upsert_search_index_job_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        task_id: Uuid,
        run_id: Option<Uuid>,
        now: OffsetDateTime,
    ) -> AppResult<()> {
        let now = format_time(now)?;
        let run_id = run_id.map(|value| value.to_string());
        query(
            r#"
            INSERT INTO search_index_jobs (
                task_id, job_kind, status, attempt_count, last_error, next_attempt_at,
                run_id, locked_at, lease_until, created_at, updated_at
            ) VALUES (?, 'task_vector_upsert', 'pending', 0, NULL, NULL, ?, NULL, NULL, ?, ?)
            ON CONFLICT(task_id) DO UPDATE SET
                status = 'pending',
                attempt_count = 0,
                last_error = NULL,
                next_attempt_at = NULL,
                run_id = excluded.run_id,
                locked_at = NULL,
                lease_until = NULL,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(task_id.to_string())
        .bind(run_id)
        .bind(&now)
        .bind(&now)
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    pub async fn claim_next_search_index_job(
        &self,
        now: OffsetDateTime,
    ) -> AppResult<Option<SearchVectorJob>> {
        let mut tx = self.pool.begin().await?;
        let now_text = format_time(now)?;
        let row = query(
            r#"
            SELECT task_id, run_id, attempt_count
            FROM search_index_jobs
            WHERE (
                status IN ('pending', 'failed')
                AND (next_attempt_at IS NULL OR next_attempt_at <= ?)
            )
               OR (
                status = 'processing'
                AND lease_until IS NOT NULL
                AND lease_until <= ?
            )
            ORDER BY updated_at ASC, task_id ASC
            LIMIT 1
            "#,
        )
        .bind(&now_text)
        .bind(&now_text)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(row) = row else {
            tx.commit().await?;
            return Ok(None);
        };

        let task_id = crate::storage::mapping::parse_uuid(
            row.get::<String, _>("task_id"),
            "search_index_jobs.task_id",
        )?;
        let run_id = row
            .get::<Option<String>, _>("run_id")
            .map(|value| crate::storage::mapping::parse_uuid(value, "search_index_jobs.run_id"))
            .transpose()?;
        let attempt_count = row.get::<i64, _>("attempt_count") + 1;
        let lease_until =
            format_time(now + time::Duration::seconds(SEARCH_INDEX_JOB_LEASE_SECONDS))?;
        query(
            r#"
            UPDATE search_index_jobs
            SET status = 'processing',
                attempt_count = ?,
                locked_at = ?,
                lease_until = ?,
                updated_at = ?
            WHERE task_id = ?
            "#,
        )
        .bind(attempt_count)
        .bind(&now_text)
        .bind(&lease_until)
        .bind(&now_text)
        .bind(task_id.to_string())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        Ok(Some(SearchVectorJob {
            task_id,
            run_id,
            attempt_count,
        }))
    }

    pub async fn complete_search_index_job(&self, task_id: Uuid) -> AppResult<()> {
        let mut tx = self.pool.begin().await?;
        let run_id = query("SELECT run_id FROM search_index_jobs WHERE task_id = ?")
            .bind(task_id.to_string())
            .fetch_optional(&mut *tx)
            .await?;
        query("DELETE FROM search_index_jobs WHERE task_id = ?")
            .bind(task_id.to_string())
            .execute(&mut *tx)
            .await?;
        if let Some(run_id) = run_id
            .and_then(|row| row.get::<Option<String>, _>("run_id"))
            .map(|value| crate::storage::mapping::parse_uuid(value, "search_index_jobs.run_id"))
            .transpose()?
        {
            let now = format_time(OffsetDateTime::now_utc())?;
            query(
                r#"
                UPDATE search_index_runs
                SET processed = processed + 1,
                    succeeded = succeeded + 1,
                    updated_at = ?
                WHERE run_id = ?
                "#,
            )
            .bind(now)
            .bind(run_id.to_string())
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn fail_search_index_job(
        &self,
        task_id: Uuid,
        error_message: &str,
        next_attempt_at: OffsetDateTime,
    ) -> AppResult<()> {
        let mut tx = self.pool.begin().await?;
        let run_id = query("SELECT run_id FROM search_index_jobs WHERE task_id = ?")
            .bind(task_id.to_string())
            .fetch_optional(&mut *tx)
            .await?;
        let updated_at = OffsetDateTime::now_utc();
        let updated_at_text = format_time(updated_at)?;
        query(
            r#"
            UPDATE search_index_jobs
            SET status = 'failed',
                last_error = ?,
                next_attempt_at = ?,
                locked_at = NULL,
                lease_until = NULL,
                updated_at = ?
            WHERE task_id = ?
            "#,
        )
        .bind(error_message)
        .bind(format_time(next_attempt_at)?)
        .bind(&updated_at_text)
        .bind(task_id.to_string())
        .execute(&mut *tx)
        .await?;
        if let Some(run_id) = run_id
            .and_then(|row| row.get::<Option<String>, _>("run_id"))
            .map(|value| crate::storage::mapping::parse_uuid(value, "search_index_jobs.run_id"))
            .transpose()?
        {
            query(
                r#"
                UPDATE search_index_runs
                SET processed = processed + 1,
                    failed = failed + 1,
                    last_error = ?,
                    updated_at = ?
                WHERE run_id = ?
                "#,
            )
            .bind(error_message)
            .bind(updated_at_text)
            .bind(run_id.to_string())
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn create_search_index_run_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        run_id: Uuid,
        trigger_kind: &str,
        scanned: usize,
        queued: usize,
        skipped: usize,
        batch_size: usize,
        now: OffsetDateTime,
    ) -> AppResult<()> {
        let now = format_time(now)?;
        query(
            r#"
            INSERT INTO search_index_runs (
                run_id, status, trigger_kind, scanned, queued, skipped,
                processed, succeeded, failed, batch_size, started_at,
                finished_at, last_error, updated_at
            ) VALUES (?, 'running', ?, ?, ?, ?, 0, 0, 0, ?, ?, NULL, NULL, ?)
            "#,
        )
        .bind(run_id.to_string())
        .bind(trigger_kind)
        .bind(scanned as i64)
        .bind(queued as i64)
        .bind(skipped as i64)
        .bind(batch_size as i64)
        .bind(&now)
        .bind(&now)
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    pub async fn list_failed_search_index_job_ids(
        &self,
        limit: Option<usize>,
    ) -> AppResult<Vec<Uuid>> {
        let limit = limit.unwrap_or(100).clamp(1, 10_000) as i64;
        let rows = query(
            r#"
            SELECT task_id
            FROM search_index_jobs
            WHERE status = 'failed'
            ORDER BY updated_at ASC, task_id ASC
            LIMIT ?
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                crate::storage::mapping::parse_uuid(
                    row.get::<String, _>("task_id"),
                    "search_index_jobs.task_id",
                )
            })
            .collect()
    }

    pub async fn list_stale_search_index_job_ids(
        &self,
        now: OffsetDateTime,
        limit: Option<usize>,
    ) -> AppResult<Vec<Uuid>> {
        let limit = limit.unwrap_or(100).clamp(1, 10_000) as i64;
        let now = format_time(now)?;
        let rows = query(
            r#"
            SELECT task_id
            FROM search_index_jobs
            WHERE status = 'processing'
              AND lease_until IS NOT NULL
              AND lease_until <= ?
            ORDER BY updated_at ASC, task_id ASC
            LIMIT ?
            "#,
        )
        .bind(now)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                crate::storage::mapping::parse_uuid(
                    row.get::<String, _>("task_id"),
                    "search_index_jobs.task_id",
                )
            })
            .collect()
    }

    pub async fn requeue_search_index_jobs_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        task_ids: &[Uuid],
        run_id: Uuid,
        now: OffsetDateTime,
    ) -> AppResult<()> {
        if task_ids.is_empty() {
            return Ok(());
        }

        let now = format_time(now)?;
        for task_id in task_ids {
            query(
                r#"
                UPDATE search_index_jobs
                SET status = 'pending',
                    attempt_count = 0,
                    last_error = NULL,
                    next_attempt_at = NULL,
                    run_id = ?,
                    locked_at = NULL,
                    lease_until = NULL,
                    updated_at = ?
                WHERE task_id = ?
                "#,
            )
            .bind(run_id.to_string())
            .bind(&now)
            .bind(task_id.to_string())
            .execute(&mut **tx)
            .await?;
        }

        Ok(())
    }

    pub async fn finish_search_index_run(
        &self,
        run_id: Uuid,
        status: &str,
        finished_at: OffsetDateTime,
        last_error: Option<&str>,
    ) -> AppResult<SearchIndexRunRecord> {
        let finished_at = format_time(finished_at)?;
        query(
            r#"
            UPDATE search_index_runs
            SET status = ?,
                finished_at = ?,
                last_error = ?,
                updated_at = ?
            WHERE run_id = ?
            "#,
        )
        .bind(status)
        .bind(&finished_at)
        .bind(last_error)
        .bind(&finished_at)
        .bind(run_id.to_string())
        .execute(&self.pool)
        .await?;
        self.get_search_index_run(run_id).await
    }

    pub async fn get_search_index_run(&self, run_id: Uuid) -> AppResult<SearchIndexRunRecord> {
        let row = query(
            r#"
            SELECT
                run_id, status, trigger_kind, scanned, queued, skipped,
                processed, succeeded, failed, batch_size, started_at,
                finished_at, last_error, updated_at
            FROM search_index_runs
            WHERE run_id = ?
            "#,
        )
        .bind(run_id.to_string())
        .fetch_one(&self.pool)
        .await?;
        map_search_index_run(row)
    }

    pub async fn latest_search_index_run(&self) -> AppResult<Option<SearchIndexRunRecord>> {
        let row = query(
            r#"
            SELECT
                run_id, status, trigger_kind, scanned, queued, skipped,
                processed, succeeded, failed, batch_size, started_at,
                finished_at, last_error, updated_at
            FROM search_index_runs
            ORDER BY updated_at DESC
            LIMIT 1
            "#,
        )
        .fetch_optional(&self.pool)
        .await?;
        row.map(map_search_index_run).transpose()
    }

    pub async fn active_search_index_run(&self) -> AppResult<Option<SearchIndexRunRecord>> {
        let row = query(
            r#"
            SELECT
                run_id, status, trigger_kind, scanned, queued, skipped,
                processed, succeeded, failed, batch_size, started_at,
                finished_at, last_error, updated_at
            FROM search_index_runs
            WHERE status = 'running'
            ORDER BY updated_at DESC
            LIMIT 1
            "#,
        )
        .fetch_optional(&self.pool)
        .await?;
        row.map(map_search_index_run).transpose()
    }

    pub async fn search_index_run_queue_stats(
        &self,
        run_id: Uuid,
    ) -> AppResult<SearchIndexRunQueueStats> {
        let row = query(
            r#"
            SELECT
                COALESCE(SUM(CASE WHEN status = 'pending' THEN 1 ELSE 0 END), 0) AS pending_count,
                COALESCE(SUM(CASE WHEN status = 'processing' THEN 1 ELSE 0 END), 0) AS processing_count,
                COALESCE(SUM(CASE WHEN status = 'failed' THEN 1 ELSE 0 END), 0) AS retrying_count
            FROM search_index_jobs
            WHERE run_id = ?
            "#,
        )
        .bind(run_id.to_string())
        .fetch_one(&self.pool)
        .await?;

        Ok(SearchIndexRunQueueStats {
            pending_count: row.get::<i64, _>("pending_count").max(0) as usize,
            processing_count: row.get::<i64, _>("processing_count").max(0) as usize,
            retrying_count: row.get::<i64, _>("retrying_count").max(0) as usize,
        })
    }

    pub async fn search_index_queue_stats(&self) -> AppResult<SearchIndexQueueStats> {
        let now = format_time(OffsetDateTime::now_utc())?;
        let row = query(
            r#"
            SELECT
                COUNT(*) AS total_count,
                COALESCE(SUM(CASE WHEN status = 'pending' THEN 1 ELSE 0 END), 0) AS pending_count,
                COALESCE(SUM(CASE WHEN status = 'processing' THEN 1 ELSE 0 END), 0) AS processing_count,
                COALESCE(SUM(CASE WHEN status = 'failed' THEN 1 ELSE 0 END), 0) AS failed_count,
                COALESCE(SUM(CASE
                    WHEN status IN ('pending', 'failed')
                     AND (next_attempt_at IS NULL OR next_attempt_at <= ?)
                    THEN 1
                    WHEN status = 'processing'
                     AND lease_until IS NOT NULL
                     AND lease_until <= ?
                    THEN 1
                    ELSE 0
                END), 0) AS due_count,
                COALESCE(SUM(CASE
                    WHEN status = 'processing'
                     AND lease_until IS NOT NULL
                     AND lease_until <= ?
                    THEN 1
                    ELSE 0
                END), 0) AS stale_processing_count,
                MIN(CASE WHEN status = 'failed' THEN next_attempt_at ELSE NULL END) AS next_retry_at,
                (
                    SELECT last_error
                    FROM search_index_jobs
                    WHERE last_error IS NOT NULL
                    ORDER BY updated_at DESC
                    LIMIT 1
                ) AS last_error
            FROM search_index_jobs
            "#,
        )
        .bind(&now)
        .bind(&now)
        .bind(&now)
        .fetch_one(&self.pool)
        .await?;

        Ok(SearchIndexQueueStats {
            total_count: row.get::<i64, _>("total_count").max(0) as usize,
            pending_count: row.get::<i64, _>("pending_count").max(0) as usize,
            processing_count: row.get::<i64, _>("processing_count").max(0) as usize,
            failed_count: row.get::<i64, _>("failed_count").max(0) as usize,
            due_count: row.get::<i64, _>("due_count").max(0) as usize,
            stale_processing_count: row.get::<i64, _>("stale_processing_count").max(0) as usize,
            next_retry_at: row
                .get::<Option<String>, _>("next_retry_at")
                .map(|value| parse_time(value, "search_index_jobs.next_retry_at"))
                .transpose()?,
            last_error: row.get("last_error"),
        })
    }

    pub async fn list_failed_search_index_jobs(
        &self,
        limit: Option<usize>,
    ) -> AppResult<Vec<SearchIndexJobRecord>> {
        let limit = limit.unwrap_or(5).clamp(1, 50) as i64;
        let rows = query(
            r#"
            SELECT
                j.task_id, t.title, j.status, j.attempt_count, j.last_error,
                j.next_attempt_at, j.locked_at, j.lease_until, j.updated_at, j.run_id
            FROM search_index_jobs j
            LEFT JOIN tasks t ON t.task_id = j.task_id
            WHERE j.status = 'failed'
            ORDER BY j.updated_at DESC, j.task_id ASC
            LIMIT ?
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(map_search_index_job_record).collect()
    }

    pub async fn pending_search_index_job_count(&self) -> AppResult<usize> {
        let row = query("SELECT COUNT(*) AS count FROM search_index_jobs")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get::<i64, _>("count").max(0) as usize)
    }

    pub async fn get_task_vector_documents(
        &self,
        task_id: Uuid,
    ) -> AppResult<Vec<TaskVectorDocument>> {
        let row = query(
            r#"
            SELECT
                t.task_id,
                t.project_id,
                p.slug AS project_slug,
                p.name AS project_name,
                p.description AS project_description,
                t.version_id,
                v.name AS version_name,
                v.description AS version_description,
                t.task_code,
                t.task_kind,
                t.title,
                t.status,
                t.priority,
                t.knowledge_status,
                t.latest_note_summary,
                (
                    SELECT ta.content
                    FROM task_activities ta
                    WHERE ta.task_id = t.task_id
                      AND ta.kind = 'attachment_ref'
                    ORDER BY ta.created_at DESC, ta.activity_id DESC
                    LIMIT 1
                ) AS latest_attachment_summary,
                t.task_search_summary,
                t.task_context_digest,
                t.updated_at
            FROM tasks t
            JOIN projects p ON p.project_id = t.project_id
            LEFT JOIN versions v ON v.version_id = t.version_id
            WHERE t.task_id = ?
            "#,
        )
        .bind(task_id.to_string())
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Ok(Vec::new());
        };

        let task_code = row.get::<Option<String>, _>("task_code");
        let title = row.get::<String, _>("title");
        let latest_note_summary = row.get::<Option<String>, _>("latest_note_summary");
        let latest_attachment_summary = row.get::<Option<String>, _>("latest_attachment_summary");
        let task_search_summary = row.get::<String, _>("task_search_summary");
        let task_context_digest = row.get::<String, _>("task_context_digest");

        let task_id_text = row.get::<String, _>("task_id");
        let project_id = row.get::<String, _>("project_id");
        let project_slug = row.get::<String, _>("project_slug");
        let project_name = row.get::<String, _>("project_name");
        let project_description = row.get::<Option<String>, _>("project_description");
        let version_id = row.get::<Option<String>, _>("version_id");
        let version_name = row.get::<Option<String>, _>("version_name");
        let version_description = row.get::<Option<String>, _>("version_description");
        let task_kind = row.get::<String, _>("task_kind");
        let status = row.get::<String, _>("status");
        let priority = row.get::<String, _>("priority");
        let knowledge_status = row.get::<String, _>("knowledge_status");
        let updated_at = row.get::<String, _>("updated_at");
        let mut documents = vec![TaskVectorDocument {
            vector_id: task_id_text.clone(),
            source_kind: "task".to_string(),
            task_id: row.get::<String, _>("task_id"),
            project_id: project_id.clone(),
            project_slug: project_slug.clone(),
            project_name: project_name.clone(),
            project_description: project_description.clone(),
            version_id: version_id.clone(),
            version_name: version_name.clone(),
            version_description: version_description.clone(),
            task_code: task_code.clone(),
            task_kind: task_kind.clone(),
            title: title.clone(),
            status: status.clone(),
            priority: priority.clone(),
            knowledge_status: knowledge_status.clone(),
            latest_note_summary: latest_note_summary.clone(),
            latest_attachment_summary: latest_attachment_summary.clone(),
            activity_id: None,
            chunk_id: None,
            chunk_index: None,
            attachment_id: None,
            task_search_summary: task_search_summary.clone(),
            task_context_digest: task_context_digest.clone(),
            updated_at: updated_at.clone(),
            document: build_task_vector_document_text(
                &project_slug,
                &project_name,
                project_description.as_deref(),
                version_name.as_deref(),
                version_description.as_deref(),
                task_code.as_deref(),
                &title,
                latest_note_summary.as_deref(),
                latest_attachment_summary.as_deref(),
                &task_search_summary,
                &task_context_digest,
            ),
        }];

        for chunk in self.list_activity_chunks_for_task(task_id).await? {
            documents.push(TaskVectorDocument {
                vector_id: format!("activity_chunk:{}", chunk.chunk_id),
                source_kind: "activity_chunk".to_string(),
                task_id: chunk.task_id,
                project_id: chunk.project_id,
                project_slug: project_slug.clone(),
                project_name: project_name.clone(),
                project_description: project_description.clone(),
                version_id: chunk.version_id,
                version_name: version_name.clone(),
                version_description: version_description.clone(),
                task_code: task_code.clone(),
                task_kind: task_kind.clone(),
                title: chunk.task_title.clone(),
                status: status.clone(),
                priority: priority.clone(),
                knowledge_status: knowledge_status.clone(),
                latest_note_summary: latest_note_summary.clone(),
                latest_attachment_summary: latest_attachment_summary.clone(),
                activity_id: Some(chunk.activity_id),
                chunk_id: Some(chunk.chunk_id),
                chunk_index: Some(chunk.chunk_index),
                attachment_id: chunk.attachment_id,
                task_search_summary: task_search_summary.clone(),
                task_context_digest: task_context_digest.clone(),
                updated_at: updated_at.clone(),
                document: build_activity_chunk_vector_document_text(
                    task_code.as_deref(),
                    &chunk.task_title,
                    &chunk.kind,
                    &chunk.summary,
                    &chunk.chunk_text,
                ),
            });
        }

        Ok(documents)
    }
}
