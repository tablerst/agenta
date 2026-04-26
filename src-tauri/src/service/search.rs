use super::*;

impl AgentaService {
    pub fn search_sidecar_autostart_enabled(&self) -> bool {
        self.search.autostart_sidecar_enabled()
    }

    pub async fn start_search_sidecar(&self) -> AppResult<()> {
        self.search.start_sidecar().await.map(|_| ())
    }

    pub async fn stop_search_sidecar(&self) -> AppResult<()> {
        self.search.stop_sidecar().await
    }

    pub async fn search_backfill(
        &self,
        limit: Option<usize>,
        batch_size: Option<usize>,
    ) -> AppResult<SearchBackfillSummary> {
        if !self.search.vector_enabled() {
            return Err(AppError::Conflict(
                "vector search is not enabled".to_string(),
            ));
        }

        let max_to_queue = limit.unwrap_or(1_000).clamp(1, 100_000);
        let batch_size = batch_size.unwrap_or(10).clamp(1, 200);
        let run_id = Uuid::new_v4();
        let mut summary = {
            let _write_guard = self.write_queue.lock().await;
            let task_ids = self.store.list_task_ids().await?;
            let mut tx = self.store.pool.begin().await?;
            let now = OffsetDateTime::now_utc();
            let queued = task_ids.len().min(max_to_queue);
            self.store
                .create_search_index_run_tx(
                    &mut tx,
                    run_id,
                    "manual_backfill",
                    task_ids.len(),
                    queued,
                    task_ids.len().saturating_sub(queued),
                    batch_size,
                    now,
                )
                .await?;
            for task_id in task_ids.iter().take(max_to_queue) {
                self.store
                    .upsert_search_index_job_tx(&mut tx, *task_id, Some(run_id), now)
                    .await?;
            }
            tx.commit().await?;
            SearchBackfillSummary {
                run_id,
                status: "running".to_string(),
                operation_kind: search_index_operation_kind("manual_backfill").to_string(),
                operation_description: search_index_operation_description("manual_backfill")
                    .to_string(),
                scanned: task_ids.len(),
                queued,
                skipped: task_ids.len().saturating_sub(queued),
                processed: 0,
                succeeded: 0,
                failed: 0,
                pending_after: 0,
                processing_error: None,
            }
        };

        summary.processing_error = self
            .search
            .process_pending_jobs_with_batch(self.store.clone(), batch_size)
            .await
            .err()
            .map(|error| error.to_string());
        summary.pending_after = self.store.pending_search_index_job_count().await?;
        let run_status = if summary.processing_error.is_some() {
            "failed"
        } else {
            "completed"
        };
        let run = self
            .store
            .finish_search_index_run(
                run_id,
                run_status,
                OffsetDateTime::now_utc(),
                summary.processing_error.as_deref(),
            )
            .await?;
        summary.status = run.status;
        summary.processed = run.processed;
        summary.succeeded = run.succeeded;
        summary.failed = run.failed;
        Ok(summary)
    }

    pub async fn retry_failed_search_index_jobs(
        &self,
        limit: Option<usize>,
        batch_size: Option<usize>,
    ) -> AppResult<SearchQueueRecoverySummary> {
        self.run_search_queue_recovery(
            "retry_failed",
            self.store.list_failed_search_index_job_ids(limit).await?,
            batch_size,
        )
        .await
    }

    pub async fn recover_stale_search_index_jobs(
        &self,
        limit: Option<usize>,
        batch_size: Option<usize>,
    ) -> AppResult<SearchQueueRecoverySummary> {
        self.run_search_queue_recovery(
            "recover_stale",
            self.store
                .list_stale_search_index_job_ids(OffsetDateTime::now_utc(), limit)
                .await?,
            batch_size,
        )
        .await
    }

    pub async fn search_index_status(&self) -> AppResult<SearchIndexStatusSummary> {
        let runtime_status = self.search.runtime_status().await;
        let queue = self.store.search_index_queue_stats().await?;
        let active_run = match self.store.active_search_index_run().await? {
            Some(run) => Some(self.search_index_run_summary(run).await?),
            None => None,
        };
        let latest_run = match self.store.latest_search_index_run().await? {
            Some(run) => Some(self.search_index_run_summary(run).await?),
            None => None,
        };
        let failed_jobs = self
            .store
            .list_failed_search_index_jobs(Some(5))
            .await?
            .into_iter()
            .map(search_index_job_summary)
            .collect();

        Ok(SearchIndexStatusSummary {
            enabled: self.search.vector_enabled(),
            vector_available: runtime_status.vector_available,
            sidecar: search_sidecar_status_label(runtime_status.sidecar).to_string(),
            total_count: queue.total_count,
            pending_count: queue.pending_count,
            processing_count: queue.processing_count,
            failed_count: queue.failed_count,
            due_count: queue.due_count,
            stale_processing_count: queue.stale_processing_count,
            next_retry_at: queue.next_retry_at,
            last_error: queue.last_error,
            active_run,
            latest_run,
            failed_jobs,
        })
    }

    pub(super) async fn run_search_queue_recovery(
        &self,
        trigger_kind: &str,
        task_ids: Vec<Uuid>,
        batch_size: Option<usize>,
    ) -> AppResult<SearchQueueRecoverySummary> {
        if !self.search.vector_enabled() {
            return Err(AppError::Conflict(
                "vector search is not enabled".to_string(),
            ));
        }

        let batch_size = batch_size.unwrap_or(10).clamp(1, 200);
        let run_id = Uuid::new_v4();
        let queued = task_ids.len();

        {
            let _write_guard = self.write_queue.lock().await;
            let mut tx = self.store.pool.begin().await?;
            let now = OffsetDateTime::now_utc();
            self.store
                .create_search_index_run_tx(
                    &mut tx,
                    run_id,
                    trigger_kind,
                    queued,
                    queued,
                    0,
                    batch_size,
                    now,
                )
                .await?;
            self.store
                .requeue_search_index_jobs_tx(&mut tx, &task_ids, run_id, now)
                .await?;
            tx.commit().await?;
        }

        let processing_error = if queued == 0 {
            None
        } else {
            self.search
                .process_pending_jobs_with_batch(self.store.clone(), batch_size)
                .await
                .err()
                .map(|error| error.to_string())
        };
        let pending_after = self.store.pending_search_index_job_count().await?;
        let run_status = if processing_error.is_some() {
            "failed"
        } else {
            "completed"
        };
        let run = self
            .store
            .finish_search_index_run(
                run_id,
                run_status,
                OffsetDateTime::now_utc(),
                processing_error.as_deref(),
            )
            .await?;

        Ok(SearchQueueRecoverySummary {
            run_id,
            status: run.status,
            operation_kind: search_index_operation_kind(&run.trigger_kind).to_string(),
            operation_description: search_index_operation_description(&run.trigger_kind)
                .to_string(),
            trigger_kind: run.trigger_kind,
            queued,
            processed: run.processed,
            succeeded: run.succeeded,
            failed: run.failed,
            pending_after,
            processing_error,
        })
    }

    pub(super) async fn search_index_run_summary(
        &self,
        record: SearchIndexRunRecord,
    ) -> AppResult<SearchIndexRunSummary> {
        let queue = self
            .store
            .search_index_run_queue_stats(record.run_id)
            .await?;
        Ok(SearchIndexRunSummary {
            run_id: record.run_id,
            status: record.status,
            operation_kind: search_index_operation_kind(&record.trigger_kind).to_string(),
            operation_description: search_index_operation_description(&record.trigger_kind)
                .to_string(),
            trigger_kind: record.trigger_kind,
            scanned: record.scanned,
            queued: record.queued,
            skipped: record.skipped,
            processed: record.processed,
            succeeded: record.succeeded,
            failed: record.failed,
            batch_size: record.batch_size,
            pending_count: queue.pending_count,
            processing_count: queue.processing_count,
            retrying_count: queue.retrying_count,
            remaining_count: queue.pending_count + queue.processing_count + queue.retrying_count,
            started_at: record.started_at,
            finished_at: record.finished_at,
            last_error: record.last_error,
            updated_at: record.updated_at,
        })
    }

    pub async fn search(&self, input: SearchInput) -> AppResult<SearchResponse> {
        let query_text = input.text.and_then(|value| clean_optional(Some(value)));
        let normalized_query = query_text.as_deref().and_then(normalize_search_query);

        let limit = input
            .limit
            .unwrap_or(DEFAULT_SEARCH_LIMIT)
            .clamp(1, MAX_SEARCH_LIMIT);
        let task_query = TaskQuery {
            project: input.project,
            version: input.version,
            status: input.status,
            task_kind: input.task_kind,
            task_code_prefix: input.task_code_prefix,
            title_prefix: input.title_prefix,
            sort_by: None,
            sort_order: None,
            all_projects: input.all_projects,
        };
        let mut filter = self.resolve_task_filter(&task_query).await?;
        filter.priority = input.priority;
        filter.knowledge_status = input.knowledge_status;
        if normalized_query.is_none()
            && filter.project_id.is_none()
            && filter.version_id.is_none()
            && filter.status.is_none()
            && filter.priority.is_none()
            && filter.knowledge_status.is_none()
            && filter.task_kind.is_none()
            && filter.task_code_prefix.is_none()
            && filter.title_prefix.is_none()
        {
            return Err(AppError::InvalidArguments(
                "search requires query text, a project context, or at least one structured filter"
                    .to_string(),
            ));
        }
        let pending_index_jobs = self.store.pending_search_index_job_count().await?;
        let retrieval_mode = if normalized_query.is_none() {
            "structured_only".to_string()
        } else {
            "lexical_only".to_string()
        };

        if normalized_query.is_none() {
            let (details, _, _) = self.collect_sorted_task_details(task_query).await?;
            return Ok(SearchResponse {
                query: query_text,
                tasks: details
                    .into_iter()
                    .filter(|detail| {
                        input
                            .priority
                            .is_none_or(|priority| detail.task.priority == priority)
                            && input.knowledge_status.is_none_or(|knowledge_status| {
                                detail.task.knowledge_status == knowledge_status
                            })
                    })
                    .take(limit)
                    .map(structured_task_hit_from_detail)
                    .collect(),
                activities: Vec::new(),
                meta: SearchMeta {
                    indexed_fields: default_indexed_fields(),
                    task_sort: "structured task filter order".to_string(),
                    activity_sort: "activities are only returned for text queries".to_string(),
                    limit_applies_per_bucket: true,
                    task_limit_applied: limit,
                    activity_limit_applied: limit,
                    default_limit: DEFAULT_SEARCH_LIMIT,
                    max_limit: MAX_SEARCH_LIMIT,
                    retrieval_mode,
                    vector_backend: self.search.vector_backend_name(),
                    vector_status: vector_status_label(
                        self.search.vector_enabled(),
                        false,
                        pending_index_jobs,
                    ),
                    pending_index_jobs,
                    semantic_attempted: false,
                    semantic_used: false,
                    semantic_error: None,
                    semantic_candidate_count: 0,
                },
            });
        }

        let normalized_query = normalized_query.expect("normalized query");
        let lexical_limit = limit
            .max(self.search.config().vector.top_k)
            .saturating_mul(4);
        let exact_fts_tasks = self
            .store
            .search_tasks(&filter, &normalized_query.fts_query, lexical_limit)
            .await?;
        let prefix_fts_tasks = match normalized_query.prefix_fts_query.as_deref() {
            Some(prefix_fts_query) => {
                self.store
                    .search_tasks(&filter, prefix_fts_query, lexical_limit)
                    .await?
            }
            None => Vec::new(),
        };
        let like_tasks = self
            .store
            .search_tasks_by_like(
                &filter,
                &normalized_query.like_text,
                &normalized_query.terms,
                lexical_limit,
            )
            .await?;
        let lexical_task_groups = match normalized_query.intent {
            crate::search::SearchIntent::Identifier => {
                vec![like_tasks, exact_fts_tasks, prefix_fts_tasks]
            }
            crate::search::SearchIntent::Phrase => vec![exact_fts_tasks, like_tasks],
            crate::search::SearchIntent::General => {
                vec![exact_fts_tasks, prefix_fts_tasks, like_tasks]
            }
        };
        let mut lexical_tasks = merge_lexical_task_rows(lexical_task_groups);
        let semantic_attempted = self.search.vector_enabled()
            && normalized_query.intent != crate::search::SearchIntent::Identifier;
        let mut semantic_error = None;
        let vector_hits = if semantic_attempted {
            match self
                .search
                .query_tasks(
                    &normalized_query.raw_text,
                    &filter,
                    self.search.config().vector.top_k,
                )
                .await
            {
                Ok(vector_hits) => vector_hits,
                Err(error) => {
                    semantic_error = Some(error.to_string());
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        };
        let semantic_candidate_count = vector_hits.len();
        let semantic_evidence_by_task = build_semantic_evidence_map(&vector_hits);
        let lexical_task_ids = lexical_tasks
            .iter()
            .map(|row| row.task_id.clone())
            .collect::<HashSet<_>>();
        let semantic_only_ids = vector_hits
            .iter()
            .filter_map(|hit| {
                (!lexical_task_ids.contains(&hit.task_id)).then_some(hit.task_id.clone())
            })
            .collect::<Vec<_>>();
        if !semantic_only_ids.is_empty() {
            let extra_rows = self.store.search_tasks_by_ids(&semantic_only_ids).await?;
            lexical_tasks.extend(
                extra_rows
                    .into_iter()
                    .filter(|row| matches_prefix_filters(row, &filter)),
            );
        }
        let activity_rows = self
            .store
            .search_activities(&filter, &normalized_query.fts_query, lexical_limit)
            .await?;
        let activity_evidence_by_task =
            build_activity_evidence_map(&activity_rows, &normalized_query.terms);
        let lexical_task_ids = lexical_tasks
            .iter()
            .map(|row| row.task_id.clone())
            .collect::<HashSet<_>>();
        let activity_task_ids = activity_rows
            .iter()
            .filter_map(|activity| {
                (!lexical_task_ids.contains(&activity.task_id)).then_some(activity.task_id.clone())
            })
            .collect::<Vec<_>>();
        if !activity_task_ids.is_empty() {
            let activity_rows = self.store.search_tasks_by_ids(&activity_task_ids).await?;
            lexical_tasks.extend(
                activity_rows
                    .into_iter()
                    .filter(|row| matches_prefix_filters(row, &filter)),
            );
            lexical_tasks = merge_lexical_task_rows(vec![lexical_tasks]);
        }
        let mut task_sources = combine_task_search_results(
            lexical_tasks,
            vector_hits,
            &normalized_query.terms,
            &activity_evidence_by_task,
            &semantic_evidence_by_task,
            limit,
        );
        let used_hybrid = task_sources
            .iter()
            .any(|hit| hit.retrieval_source == "hybrid" || hit.retrieval_source == "semantic");
        if semantic_candidate_count > 0 {
            rerank_task_hits(&mut task_sources);
        }
        let activities = activity_rows
            .into_iter()
            .take(limit)
            .map(|activity| {
                let matched_fields = matched_field_names(
                    &normalized_query.terms,
                    [
                        ("activity_search_summary", Some(activity.summary.as_str())),
                        ("activity_search_text", Some(activity.search_text.as_str())),
                    ],
                );
                let evidence = build_search_evidence(
                    &normalized_query.terms,
                    [
                        ("activity_search_text", Some(activity.search_text.as_str())),
                        ("activity_search_summary", Some(activity.summary.as_str())),
                    ],
                );
                ActivitySearchHit {
                    activity_id: activity.activity_id,
                    task_id: activity.task_id,
                    project_id: activity.project_id,
                    version_id: activity.version_id,
                    task_title: activity.task_title,
                    kind: activity.kind,
                    summary: activity.summary,
                    retrieval_source: "lexical".to_string(),
                    score: Some(activity.score),
                    matched_fields,
                    evidence_source: evidence.as_ref().map(|item| item.source.clone()),
                    evidence_snippet: evidence.as_ref().map(|item| item.snippet.clone()),
                    evidence_chunk_id: Some(activity.chunk_id),
                    evidence_attachment_id: activity.attachment_id,
                }
            })
            .collect::<Vec<_>>();
        self.search.trigger_index_worker(self.store.clone());

        Ok(SearchResponse {
            query: Some(normalized_query.raw_text.clone()),
            tasks: std::mem::take(&mut task_sources),
            activities,
            meta: SearchMeta {
                indexed_fields: default_indexed_fields(),
                task_sort: if used_hybrid {
                    "weighted RRF over lexical cascade (fts exact/prefix plus like fallback) and chroma semantic rank, followed by evidence-aware rerank".to_string()
                } else if normalized_query.intent == crate::search::SearchIntent::Identifier {
                    "identifier-biased lexical cascade over sqlite like fallback plus sqlite fts exact/prefix".to_string()
                } else {
                    "lexical cascade over sqlite fts5 exact/prefix plus sqlite like fallback with recency tiebreaks".to_string()
                },
                activity_sort: "sqlite fts5 bm25 with structured task filters applied".to_string(),
                limit_applies_per_bucket: true,
                task_limit_applied: limit,
                activity_limit_applied: limit,
                default_limit: DEFAULT_SEARCH_LIMIT,
                max_limit: MAX_SEARCH_LIMIT,
                retrieval_mode: if used_hybrid {
                    "hybrid".to_string()
                } else {
                    "lexical_only".to_string()
                },
                vector_backend: self.search.vector_backend_name(),
                vector_status: vector_status_label(
                    self.search.vector_enabled(),
                    used_hybrid,
                    pending_index_jobs,
                ),
                pending_index_jobs,
                semantic_attempted,
                semantic_used: used_hybrid,
                semantic_error,
                semantic_candidate_count,
            },
        })
    }

    pub async fn get_search_evidence(
        &self,
        input: SearchEvidenceInput,
    ) -> AppResult<SearchEvidenceDetail> {
        if let Some(chunk_id) = input
            .chunk_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let chunk = self.store.get_activity_chunk(chunk_id).await?;
            return Ok(SearchEvidenceDetail {
                source_kind: "activity_chunk".to_string(),
                task_id: chunk.task_id,
                project_id: chunk.project_id,
                version_id: chunk.version_id,
                task_title: chunk.task_title,
                activity_id: Some(chunk.activity_id),
                chunk_id: Some(chunk.chunk_id),
                chunk_index: Some(chunk.chunk_index),
                attachment_id: chunk.attachment_id,
                activity_kind: Some(chunk.kind),
                summary: chunk.summary,
                text: chunk.chunk_text,
            });
        }

        if let Some(attachment_id) = input
            .attachment_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let attachment = self.store.get_attachment_by_ref(attachment_id).await?;
            let task = self
                .store
                .get_task_with_stats_by_ref(&attachment.task_id.to_string())
                .await?
                .0;
            let attachment_path = self.store.attachments_dir.join(&attachment.storage_path);
            let bytes = fs::read(&attachment_path).await?;
            let text = self
                .store
                .extract_attachment_search_text(
                    &bytes,
                    &attachment.mime,
                    &attachment.original_filename,
                )
                .unwrap_or_else(|| attachment.summary.clone());
            return Ok(SearchEvidenceDetail {
                source_kind: "attachment".to_string(),
                task_id: attachment.task_id.to_string(),
                project_id: task.project_id.to_string(),
                version_id: task.version_id.map(|value| value.to_string()),
                task_title: task.title,
                activity_id: None,
                chunk_id: None,
                chunk_index: None,
                attachment_id: Some(attachment.attachment_id.to_string()),
                activity_kind: None,
                summary: attachment.summary,
                text,
            });
        }

        Err(AppError::InvalidArguments(
            "search evidence lookup requires chunk_id or attachment_id".to_string(),
        ))
    }

    pub(super) async fn queue_task_search_jobs_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        task_ids: &[Uuid],
    ) -> AppResult<usize> {
        if !self.search.vector_enabled() || task_ids.is_empty() {
            return Ok(0);
        }

        let now = OffsetDateTime::now_utc();
        for task_id in task_ids {
            self.store
                .upsert_search_index_job_tx(tx, *task_id, None, now)
                .await?;
        }
        Ok(task_ids.len())
    }

    pub(super) async fn queue_project_task_search_jobs_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        project_id: Uuid,
    ) -> AppResult<usize> {
        let task_ids = self
            .store
            .list_task_ids_by_project_tx(tx, project_id)
            .await?;
        self.queue_task_search_jobs_tx(tx, &task_ids).await
    }

    pub(super) async fn queue_version_task_search_jobs_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        version_id: Uuid,
    ) -> AppResult<usize> {
        let task_ids = self
            .store
            .list_task_ids_by_version_tx(tx, version_id)
            .await?;
        self.queue_task_search_jobs_tx(tx, &task_ids).await
    }
}
