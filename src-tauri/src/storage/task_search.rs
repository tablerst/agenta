use super::*;

impl SqliteStore {
    pub async fn search_tasks(
        &self,
        filter: &TaskListFilter,
        query_text: &str,
        limit: usize,
    ) -> AppResult<Vec<TaskLexicalSearchRow>> {
        let mut builder = QueryBuilder::<Sqlite>::new(
            r#"
            SELECT
                t.task_id,
                t.task_code,
                t.task_kind,
                t.title,
                t.status,
                t.priority,
                t.knowledge_status,
                t.task_search_summary,
                t.task_context_digest,
                t.latest_note_summary,
                bm25(tasks_fts, 8.0, 10.0, 1.0, 1.25, 0.75, 1.5) AS lexical_score,
                max(
                    t.updated_at,
                    COALESCE(
                        (
                            SELECT MAX(ta.created_at)
                            FROM task_activities ta
                            WHERE ta.task_id = t.task_id
                        ),
                        t.updated_at
                    )
                ) AS latest_activity_at
            FROM tasks_fts f
            JOIN tasks t ON t.rowid = f.rowid
            WHERE tasks_fts MATCH
            "#,
        );
        builder.push_bind(query_text);
        push_task_filter_predicates(&mut builder, filter);
        builder.push(" ORDER BY lexical_score ASC, latest_activity_at DESC, t.task_id ASC LIMIT ");
        builder.push_bind(limit as i64);

        let rows = builder.build().fetch_all(&self.pool).await?;
        rows.into_iter()
            .enumerate()
            .map(|(index, row)| {
                Ok(TaskLexicalSearchRow {
                    task_id: row.get::<String, _>("task_id"),
                    task_code: row.get::<Option<String>, _>("task_code"),
                    task_kind: row.get::<String, _>("task_kind"),
                    title: row.get::<String, _>("title"),
                    status: row.get::<String, _>("status"),
                    priority: row.get::<String, _>("priority"),
                    knowledge_status: row.get::<String, _>("knowledge_status"),
                    task_search_summary: row.get::<String, _>("task_search_summary"),
                    task_context_digest: row.get::<String, _>("task_context_digest"),
                    latest_note_summary: row.get::<Option<String>, _>("latest_note_summary"),
                    lexical_score: row.get::<f64, _>("lexical_score"),
                    lexical_rank: index,
                    latest_activity_at: parse_time(
                        row.get("latest_activity_at"),
                        "latest_activity_at",
                    )?,
                })
            })
            .collect()
    }

    pub async fn search_tasks_by_like(
        &self,
        filter: &TaskListFilter,
        raw_text: &str,
        terms: &[String],
        limit: usize,
    ) -> AppResult<Vec<TaskLexicalSearchRow>> {
        let normalized_raw_text = raw_text.trim();
        if normalized_raw_text.is_empty() || terms.is_empty() {
            return Ok(Vec::new());
        }

        let raw_exact = normalized_raw_text.to_string();
        let raw_prefix = format!("{}%", escape_like_pattern(normalized_raw_text));
        let raw_contains = format!("%{}%", escape_like_pattern(normalized_raw_text));

        let mut builder = QueryBuilder::<Sqlite>::new(
            r#"
            SELECT
                t.task_id,
                t.task_code,
                t.task_kind,
                t.title,
                t.status,
                t.priority,
                t.knowledge_status,
                t.task_search_summary,
                t.task_context_digest,
                t.latest_note_summary,
                CAST(
                    CASE
                        WHEN lower(COALESCE(t.task_code, '')) = 
            "#,
        );
        builder.push_bind(raw_exact.clone());
        builder.push(
            r#"
                        THEN 0
                        WHEN lower(COALESCE(t.task_code, '')) LIKE 
            "#,
        );
        builder.push_bind(raw_prefix.clone());
        builder.push(
            r#"
                        ESCAPE '\'
                        THEN 1
                        WHEN lower(t.title) = 
            "#,
        );
        builder.push_bind(raw_exact);
        builder.push(
            r#"
                        THEN 2
                        WHEN lower(t.title) LIKE 
            "#,
        );
        builder.push_bind(raw_prefix);
        builder.push(
            r#"
                        ESCAPE '\'
                        THEN 3
                        WHEN lower(t.title) LIKE 
            "#,
        );
        builder.push_bind(raw_contains.clone());
        builder.push(
            r#"
                        ESCAPE '\'
                        THEN 4
                        WHEN lower(t.task_search_summary) LIKE 
            "#,
        );
        builder.push_bind(raw_contains.clone());
        builder.push(
            r#"
                        ESCAPE '\'
                        THEN 5
                        WHEN lower(t.task_context_digest) LIKE 
            "#,
        );
        builder.push_bind(raw_contains.clone());
        builder.push(
            r#"
                        ESCAPE '\'
                        THEN 6
                        WHEN lower(COALESCE(t.latest_note_summary, '')) LIKE 
            "#,
        );
        builder.push_bind(raw_contains);
        builder.push(
            r#"
                        ESCAPE '\'
                        THEN 7
                        ELSE 8
                    END AS REAL
                ) AS lexical_score,
                max(
                    t.updated_at,
                    COALESCE(
                        (
                            SELECT MAX(ta.created_at)
                            FROM task_activities ta
                            WHERE ta.task_id = t.task_id
                        ),
                        t.updated_at
                    )
                ) AS latest_activity_at
            FROM tasks t
            WHERE 1 = 1
            "#,
        );

        for term in terms {
            let pattern = format!("%{}%", escape_like_pattern(term));
            builder.push(" AND (lower(COALESCE(t.task_code, '')) LIKE ");
            builder.push_bind(pattern.clone());
            builder.push(" ESCAPE '\\' OR lower(t.title) LIKE ");
            builder.push_bind(pattern.clone());
            builder.push(" ESCAPE '\\' OR lower(t.task_search_summary) LIKE ");
            builder.push_bind(pattern.clone());
            builder.push(" ESCAPE '\\' OR lower(t.task_context_digest) LIKE ");
            builder.push_bind(pattern.clone());
            builder.push(" ESCAPE '\\' OR lower(COALESCE(t.latest_note_summary, '')) LIKE ");
            builder.push_bind(pattern);
            builder.push(" ESCAPE '\\')");
        }

        push_task_filter_predicates(&mut builder, filter);
        builder.push(" ORDER BY lexical_score ASC, latest_activity_at DESC, t.task_id ASC LIMIT ");
        builder.push_bind(limit as i64);

        let rows = builder.build().fetch_all(&self.pool).await?;
        rows.into_iter()
            .enumerate()
            .map(|(index, row)| {
                Ok(TaskLexicalSearchRow {
                    task_id: row.get::<String, _>("task_id"),
                    task_code: row.get::<Option<String>, _>("task_code"),
                    task_kind: row.get::<String, _>("task_kind"),
                    title: row.get::<String, _>("title"),
                    status: row.get::<String, _>("status"),
                    priority: row.get::<String, _>("priority"),
                    knowledge_status: row.get::<String, _>("knowledge_status"),
                    task_search_summary: row.get::<String, _>("task_search_summary"),
                    task_context_digest: row.get::<String, _>("task_context_digest"),
                    latest_note_summary: row.get::<Option<String>, _>("latest_note_summary"),
                    lexical_score: row.get::<f64, _>("lexical_score"),
                    lexical_rank: index,
                    latest_activity_at: parse_time(
                        row.get("latest_activity_at"),
                        "latest_activity_at",
                    )?,
                })
            })
            .collect()
    }

    pub async fn search_tasks_by_ids(
        &self,
        task_ids: &[String],
    ) -> AppResult<Vec<TaskLexicalSearchRow>> {
        if task_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut builder = QueryBuilder::<Sqlite>::new(
            r#"
            SELECT
                t.task_id,
                t.task_code,
                t.task_kind,
                t.title,
                t.status,
                t.priority,
                t.knowledge_status,
                t.task_search_summary,
                t.task_context_digest,
                t.latest_note_summary,
                0.0 AS lexical_score,
                max(
                    t.updated_at,
                    COALESCE(
                        (
                            SELECT MAX(ta.created_at)
                            FROM task_activities ta
                            WHERE ta.task_id = t.task_id
                        ),
                        t.updated_at
                    )
                ) AS latest_activity_at
            FROM tasks t
            WHERE t.task_id IN (
            "#,
        );
        let mut separated = builder.separated(", ");
        for task_id in task_ids {
            separated.push_bind(task_id);
        }
        builder.push(")");

        let rows = builder.build().fetch_all(&self.pool).await?;
        rows.into_iter()
            .map(|row| {
                Ok(TaskLexicalSearchRow {
                    task_id: row.get::<String, _>("task_id"),
                    task_code: row.get::<Option<String>, _>("task_code"),
                    task_kind: row.get::<String, _>("task_kind"),
                    title: row.get::<String, _>("title"),
                    status: row.get::<String, _>("status"),
                    priority: row.get::<String, _>("priority"),
                    knowledge_status: row.get::<String, _>("knowledge_status"),
                    task_search_summary: row.get::<String, _>("task_search_summary"),
                    task_context_digest: row.get::<String, _>("task_context_digest"),
                    latest_note_summary: row.get::<Option<String>, _>("latest_note_summary"),
                    lexical_score: 0.0,
                    lexical_rank: usize::MAX,
                    latest_activity_at: parse_time(
                        row.get("latest_activity_at"),
                        "latest_activity_at",
                    )?,
                })
            })
            .collect()
    }

    pub async fn search_activities(
        &self,
        filter: &TaskListFilter,
        query_text: &str,
        limit: usize,
    ) -> AppResult<Vec<ActivityLexicalSearchRow>> {
        let mut builder = QueryBuilder::<Sqlite>::new(
            r#"
            WITH matched_chunks AS (
                SELECT
                    c.activity_id,
                    a.task_id,
                    a.kind,
                    a.activity_search_summary,
                    c.chunk_text,
                    c.chunk_index,
                    a.created_at,
                    bm25(task_activity_chunks_fts, 1.0) AS lexical_score
                FROM task_activity_chunks_fts f
                JOIN task_activity_chunks c ON c.rowid = f.rowid
                JOIN task_activities a ON a.activity_id = c.activity_id
                JOIN tasks t ON t.task_id = a.task_id
                WHERE task_activity_chunks_fts MATCH
            "#,
        );
        builder.push_bind(query_text);
        push_task_filter_predicates(&mut builder, filter);
        builder.push(
            r#"
            ),
            ranked_chunks AS (
                SELECT
                    *,
                    ROW_NUMBER() OVER (
                        PARTITION BY activity_id
                        ORDER BY lexical_score ASC, chunk_index ASC
                    ) AS chunk_rank
                FROM matched_chunks
            )
            SELECT
                activity_id,
                task_id,
                kind,
                activity_search_summary,
                chunk_text,
                lexical_score
            FROM ranked_chunks
            WHERE chunk_rank = 1
            ORDER BY lexical_score ASC, created_at DESC, activity_id ASC LIMIT
            "#,
        );
        builder.push_bind(limit as i64);

        let rows = builder.build().fetch_all(&self.pool).await?;
        Ok(rows
            .into_iter()
            .map(|row| ActivityLexicalSearchRow {
                activity_id: row.get::<String, _>("activity_id"),
                task_id: row.get::<String, _>("task_id"),
                kind: row.get::<String, _>("kind"),
                summary: row.get::<String, _>("activity_search_summary"),
                search_text: row.get::<String, _>("chunk_text"),
                score: row.get::<f64, _>("lexical_score"),
            })
            .collect())
    }
}
