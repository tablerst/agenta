use super::*;

pub(super) fn task_detail_from_parts(
    task: Task,
    note_count: i64,
    attachment_count: i64,
    latest_activity_at: OffsetDateTime,
    parent_task_id: Option<Uuid>,
    child_count: i64,
    open_blocker_count: i64,
    blocking_count: i64,
) -> TaskDetail {
    let ready_to_start = task_ready_to_start(&task, open_blocker_count);
    TaskDetail {
        task,
        note_count,
        attachment_count,
        latest_activity_at,
        parent_task_id,
        child_count,
        open_blocker_count,
        blocking_count,
        ready_to_start,
    }
}

pub(super) fn build_task_context_digest_from_detail(detail: &TaskDetail) -> String {
    let digest = format!(
        "status={} priority={} task_code={} task_kind={} knowledge_status={} latest_note_summary={} ready_to_start={} parent_task_id={} child_count={} open_blocker_count={} blocking_count={} title={} summary={} description={}",
        detail.task.status,
        detail.task.priority,
        detail.task.task_code.as_deref().unwrap_or(""),
        detail.task.task_kind,
        detail.task.knowledge_status,
        detail.task.latest_note_summary.as_deref().unwrap_or(""),
        detail.ready_to_start,
        detail
            .parent_task_id
            .map(|value| value.to_string())
            .unwrap_or_default(),
        detail.child_count,
        detail.open_blocker_count,
        detail.blocking_count,
        detail.task.title,
        detail.task.summary.as_deref().unwrap_or(""),
        detail.task.description.as_deref().unwrap_or("")
    );
    if digest.chars().count() <= 320 {
        digest
    } else {
        let mut output = digest.chars().take(319).collect::<String>();
        output.push_str("...");
        output
    }
}

pub(super) fn build_task_list_summary(details: &[TaskDetail]) -> TaskListSummary {
    let mut summary = TaskListSummary {
        total: details.len(),
        status_counts: TaskStatusCounts {
            draft: 0,
            ready: 0,
            in_progress: 0,
            blocked: 0,
            done: 0,
            cancelled: 0,
        },
        knowledge_counts: TaskKnowledgeCounts {
            empty: 0,
            working: 0,
            reusable: 0,
        },
        kind_counts: TaskKindCounts {
            standard: 0,
            context: 0,
            index: 0,
        },
        ready_to_start_count: 0,
    };

    for detail in details {
        match detail.task.status {
            TaskStatus::Draft => summary.status_counts.draft += 1,
            TaskStatus::Ready => summary.status_counts.ready += 1,
            TaskStatus::InProgress => summary.status_counts.in_progress += 1,
            TaskStatus::Blocked => summary.status_counts.blocked += 1,
            TaskStatus::Done => summary.status_counts.done += 1,
            TaskStatus::Cancelled => summary.status_counts.cancelled += 1,
        }
        match detail.task.knowledge_status {
            KnowledgeStatus::Empty => summary.knowledge_counts.empty += 1,
            KnowledgeStatus::Working => summary.knowledge_counts.working += 1,
            KnowledgeStatus::Reusable => summary.knowledge_counts.reusable += 1,
        }
        match detail.task.task_kind {
            TaskKind::Standard => summary.kind_counts.standard += 1,
            TaskKind::Context => summary.kind_counts.context += 1,
            TaskKind::Index => summary.kind_counts.index += 1,
        }
        if detail.ready_to_start {
            summary.ready_to_start_count += 1;
        }
    }

    summary
}

pub(super) fn default_task_sort(version_ref: Option<&str>, details: &[TaskDetail]) -> TaskSortBy {
    if version_ref.is_some()
        && details.iter().any(|detail| {
            detail
                .task
                .task_code
                .as_deref()
                .is_some_and(|value| !value.is_empty())
        })
    {
        TaskSortBy::TaskCode
    } else {
        TaskSortBy::CreatedAt
    }
}

pub(super) fn sort_task_details(
    details: &mut [TaskDetail],
    sort_by: TaskSortBy,
    sort_order: SortOrder,
) {
    details.sort_by(|left, right| {
        let ordering = match sort_by {
            TaskSortBy::CreatedAt => left.task.created_at.cmp(&right.task.created_at),
            TaskSortBy::UpdatedAt => left.task.updated_at.cmp(&right.task.updated_at),
            TaskSortBy::LatestActivityAt => left.latest_activity_at.cmp(&right.latest_activity_at),
            TaskSortBy::TaskCode => compare_task_code_fields(left, right),
            TaskSortBy::Title => compare_text(
                left.task.title.as_str(),
                right.task.title.as_str(),
                left.task.task_id,
                right.task.task_id,
            ),
        };
        match sort_order {
            SortOrder::Asc => ordering,
            SortOrder::Desc => ordering.reverse(),
        }
    });
}

pub(super) fn compare_task_code_fields(
    left: &TaskDetail,
    right: &TaskDetail,
) -> std::cmp::Ordering {
    let left_code = left.task.task_code.as_deref();
    let right_code = right.task.task_code.as_deref();
    match (left_code, right_code) {
        (Some(left_code), Some(right_code)) => compare_task_codes(
            left_code,
            right_code,
            left.task.title.as_str(),
            right.task.title.as_str(),
            left.task.task_id,
            right.task.task_id,
        ),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => compare_text(
            left.task.title.as_str(),
            right.task.title.as_str(),
            left.task.task_id,
            right.task.task_id,
        ),
    }
}

pub(super) fn compare_task_codes(
    left: &str,
    right: &str,
    left_title: &str,
    right_title: &str,
    left_id: Uuid,
    right_id: Uuid,
) -> std::cmp::Ordering {
    let left_raw = left.trim().to_ascii_lowercase();
    let right_raw = right.trim().to_ascii_lowercase();
    let left_parts = task_code_parts(&left_raw);
    let right_parts = task_code_parts(&right_raw);
    left_parts
        .0
        .cmp(&right_parts.0)
        .then_with(|| left_parts.1.cmp(&right_parts.1))
        .then_with(|| left_raw.cmp(&right_raw))
        .then_with(|| compare_text(left_title, right_title, left_id, right_id))
}

pub(super) fn task_code_parts(value: &str) -> (String, u64) {
    if let Some((prefix, suffix)) = value.rsplit_once('-') {
        let prefix = prefix.trim();
        let suffix = suffix.trim();
        if !prefix.is_empty() && !suffix.is_empty() && suffix.chars().all(|ch| ch.is_ascii_digit())
        {
            return (
                prefix.to_string(),
                suffix.parse::<u64>().unwrap_or(u64::MAX),
            );
        }
    }
    (value.trim().to_string(), u64::MAX)
}

pub(super) fn compare_text(
    left: &str,
    right: &str,
    left_id: Uuid,
    right_id: Uuid,
) -> std::cmp::Ordering {
    left.trim()
        .to_ascii_lowercase()
        .cmp(&right.trim().to_ascii_lowercase())
        .then_with(|| left_id.cmp(&right_id))
}

pub(super) fn paginate_presorted_by_cursor<T, FCreatedAt, FId>(
    items: Vec<T>,
    page: PageRequest,
    created_at: FCreatedAt,
    id: FId,
) -> PageResult<T>
where
    FCreatedAt: Fn(&T) -> OffsetDateTime,
    FId: Fn(&T) -> Uuid,
{
    let start_index = page.cursor.and_then(|cursor| {
        items
            .iter()
            .position(|item| created_at(item) == cursor.created_at && id(item) == cursor.id)
            .map(|index| index + 1)
    });
    let mut items = if let Some(start_index) = start_index {
        items.into_iter().skip(start_index).collect::<Vec<_>>()
    } else {
        items
    };

    let Some(limit) = page.limit.map(|value| value.clamp(1, 50)) else {
        return PageResult {
            items,
            limit: None,
            next_cursor: None,
            has_more: false,
        };
    };

    let has_more = items.len() > limit;
    if has_more {
        items.truncate(limit + 1);
    }

    let next_cursor = if has_more {
        let last_visible = &items[limit - 1];
        Some(PageCursor {
            created_at: created_at(last_visible),
            id: id(last_visible),
        })
    } else {
        None
    };

    if has_more {
        items.truncate(limit);
    }

    PageResult {
        items,
        limit: Some(limit),
        next_cursor,
        has_more,
    }
}

pub(super) fn paginate_by_created_at<T, FCreatedAt, FId>(
    mut items: Vec<T>,
    page: PageRequest,
    created_at: FCreatedAt,
    id: FId,
) -> PageResult<T>
where
    FCreatedAt: Fn(&T) -> OffsetDateTime,
    FId: Fn(&T) -> Uuid,
{
    items.sort_by(|left, right| {
        created_at(right)
            .cmp(&created_at(left))
            .then_with(|| id(right).cmp(&id(left)))
    });

    if let Some(cursor) = page.cursor {
        items.retain(|item| {
            let item_created_at = created_at(item);
            let item_id = id(item);
            item_created_at < cursor.created_at
                || (item_created_at == cursor.created_at && item_id < cursor.id)
        });
    }

    let Some(limit) = page.limit.map(|value| value.clamp(1, 50)) else {
        return PageResult {
            items,
            limit: None,
            next_cursor: None,
            has_more: false,
        };
    };

    let has_more = items.len() > limit;
    if has_more {
        items.truncate(limit + 1);
    }

    let next_cursor = if has_more {
        let last_visible = &items[limit - 1];
        Some(PageCursor {
            created_at: created_at(last_visible),
            id: id(last_visible),
        })
    } else {
        None
    };

    if has_more {
        items.truncate(limit);
    }

    PageResult {
        items,
        limit: Some(limit),
        next_cursor,
        has_more,
    }
}
