use super::*;

pub(super) fn push_task_filter_predicates(
    builder: &mut QueryBuilder<'_, Sqlite>,
    filter: &TaskListFilter,
) {
    if let Some(project_id) = filter.project_id {
        builder
            .push(" AND t.project_id = ")
            .push_bind(project_id.to_string());
    }
    if let Some(version_id) = filter.version_id {
        builder
            .push(" AND t.version_id = ")
            .push_bind(version_id.to_string());
    }
    if let Some(status) = filter.status {
        builder
            .push(" AND t.status = ")
            .push_bind(status.to_string());
    }
    if let Some(priority) = filter.priority {
        builder
            .push(" AND t.priority = ")
            .push_bind(priority.to_string());
    }
    if let Some(knowledge_status) = filter.knowledge_status {
        builder
            .push(" AND t.knowledge_status = ")
            .push_bind(knowledge_status.to_string());
    }
    if let Some(task_kind) = filter.task_kind {
        builder
            .push(" AND t.task_kind = ")
            .push_bind(task_kind.to_string());
    }
    if let Some(task_code_prefix) = filter.task_code_prefix.as_deref() {
        builder
            .push(" AND t.task_code LIKE ")
            .push_bind(format!("{task_code_prefix}%"));
    }
    if let Some(title_prefix) = filter.title_prefix.as_deref() {
        builder
            .push(" AND t.title LIKE ")
            .push_bind(format!("{title_prefix}%"));
    }
}

pub(super) fn escape_like_pattern(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

pub(super) fn map_optional_time(
    value: Option<String>,
    field: &str,
) -> AppResult<Option<OffsetDateTime>> {
    value.map(|value| parse_time(value, field)).transpose()
}

pub(super) fn map_optional_uuid(value: Option<String>, field: &str) -> AppResult<Option<Uuid>> {
    value
        .map(|value| crate::storage::mapping::parse_uuid(value, field))
        .transpose()
}

pub(super) fn map_search_index_run(row: SqliteRow) -> AppResult<SearchIndexRunRecord> {
    Ok(SearchIndexRunRecord {
        run_id: crate::storage::mapping::parse_uuid(row.get("run_id"), "search_index_runs.run_id")?,
        status: row.get("status"),
        trigger_kind: row.get("trigger_kind"),
        scanned: row.get::<i64, _>("scanned").max(0) as usize,
        queued: row.get::<i64, _>("queued").max(0) as usize,
        skipped: row.get::<i64, _>("skipped").max(0) as usize,
        processed: row.get::<i64, _>("processed").max(0) as usize,
        succeeded: row.get::<i64, _>("succeeded").max(0) as usize,
        failed: row.get::<i64, _>("failed").max(0) as usize,
        batch_size: row.get::<i64, _>("batch_size").max(0) as usize,
        started_at: parse_time(row.get("started_at"), "search_index_runs.started_at")?,
        finished_at: map_optional_time(row.get("finished_at"), "search_index_runs.finished_at")?,
        last_error: row.get("last_error"),
        updated_at: parse_time(row.get("updated_at"), "search_index_runs.updated_at")?,
    })
}

pub(super) fn map_search_index_job_record(row: SqliteRow) -> AppResult<SearchIndexJobRecord> {
    Ok(SearchIndexJobRecord {
        task_id: crate::storage::mapping::parse_uuid(
            row.get("task_id"),
            "search_index_jobs.task_id",
        )?,
        title: row.get("title"),
        status: row.get("status"),
        attempt_count: row.get("attempt_count"),
        last_error: row.get("last_error"),
        next_attempt_at: map_optional_time(
            row.get("next_attempt_at"),
            "search_index_jobs.next_attempt_at",
        )?,
        locked_at: map_optional_time(row.get("locked_at"), "search_index_jobs.locked_at")?,
        lease_until: map_optional_time(row.get("lease_until"), "search_index_jobs.lease_until")?,
        updated_at: parse_time(row.get("updated_at"), "search_index_jobs.updated_at")?,
        run_id: map_optional_uuid(row.get("run_id"), "search_index_jobs.run_id")?,
    })
}

pub(super) fn map_task_with_stats(
    row: sqlx::sqlite::SqliteRow,
) -> AppResult<(Task, i64, i64, OffsetDateTime, Option<Uuid>, i64, i64, i64)> {
    let note_count = row.get::<i64, _>("note_count");
    let attachment_count = row.get::<i64, _>("attachment_count");
    let latest_activity_at = parse_time(row.get("latest_activity_at"), "latest_activity_at")?;
    let parent_task_id = row
        .get::<Option<String>, _>("parent_task_id")
        .map(|value| crate::storage::mapping::parse_uuid(value, "parent_task_id"))
        .transpose()?;
    let child_count = row.get::<i64, _>("child_count");
    let open_blocker_count = row.get::<i64, _>("open_blocker_count");
    let blocking_count = row.get::<i64, _>("blocking_count");
    let task = map_task(row)?;
    Ok((
        task,
        note_count,
        attachment_count,
        latest_activity_at,
        parent_task_id,
        child_count,
        open_blocker_count,
        blocking_count,
    ))
}
