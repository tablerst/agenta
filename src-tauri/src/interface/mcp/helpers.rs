use super::*;

pub(super) fn format_timestamp(value: OffsetDateTime) -> String {
    value
        .format(&Rfc3339)
        .unwrap_or_else(|_| value.unix_timestamp().to_string())
}

pub(super) fn note_kind_for_activity(activity: &TaskActivity) -> NoteKind {
    activity
        .metadata_json
        .get("note_kind")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<NoteKind>().ok())
        .unwrap_or_default()
}

pub(super) fn required(value: Option<String>, field: &str) -> Result<String, ErrorData> {
    match value {
        Some(value) if !value.trim().is_empty() => Ok(value.trim().to_string()),
        _ => Err(ErrorData::invalid_params(
            format!("missing required field: {field}"),
            None,
        )),
    }
}

pub(super) fn required_text(value: String, field: &str) -> Result<String, ErrorData> {
    required(Some(value), field)
}

pub(super) fn optional_trimmed(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

pub(super) fn page_request(
    limit: Option<usize>,
    cursor: Option<String>,
) -> Result<PageRequest, ErrorData> {
    if cursor.is_some() && limit.is_none() {
        return Err(ErrorData::invalid_params(
            "cursor requires limit to be provided".to_string(),
            None,
        ));
    }

    Ok(PageRequest {
        limit,
        cursor: cursor.map(decode_cursor).transpose()?,
    })
}

pub(super) fn page_info<T>(page: &PageResult<T>, sort_by: &str) -> PageInfo {
    PageInfo {
        limit: page.limit,
        next_cursor: page
            .next_cursor
            .as_ref()
            .map(|cursor| encode_cursor(cursor, None, None)),
        has_more: page.has_more,
        sort_by: sort_by.to_string(),
        sort_order: "desc".to_string(),
    }
}

pub(super) fn task_page_info(page: &TaskListPageResult) -> PageInfo {
    PageInfo {
        limit: page.limit,
        next_cursor: page
            .next_cursor
            .as_ref()
            .map(|cursor| encode_cursor(cursor, Some(page.sort_by), Some(page.sort_order))),
        has_more: page.has_more,
        sort_by: page.sort_by.to_string(),
        sort_order: page.sort_order.to_string(),
    }
}

pub(super) fn decode_cursor(cursor: String) -> Result<PageCursor, ErrorData> {
    let bytes = URL_SAFE_NO_PAD.decode(cursor.as_bytes()).map_err(|error| {
        ErrorData::invalid_params(format!("invalid cursor encoding: {error}"), None)
    })?;
    let payload: CursorPayload = serde_json::from_slice(&bytes).map_err(|error| {
        ErrorData::invalid_params(format!("invalid cursor payload: {error}"), None)
    })?;
    let created_at = OffsetDateTime::parse(&payload.created_at, &Rfc3339).map_err(|error| {
        ErrorData::invalid_params(format!("invalid cursor timestamp: {error}"), None)
    })?;
    let id = Uuid::parse_str(&payload.id)
        .map_err(|error| ErrorData::invalid_params(format!("invalid cursor id: {error}"), None))?;
    Ok(PageCursor { created_at, id })
}

pub(super) fn encode_cursor(
    cursor: &PageCursor,
    sort_by: Option<TaskSortBy>,
    sort_order: Option<SortOrder>,
) -> String {
    let payload = CursorPayload {
        created_at: format_timestamp(cursor.created_at),
        id: cursor.id.to_string(),
        sort_by: sort_by.map(|value| value.to_string()),
        sort_order: sort_order.map(|value| value.to_string()),
    };
    let bytes = serde_json::to_vec(&payload).expect("cursor payload json");
    URL_SAFE_NO_PAD.encode(bytes)
}

pub(super) fn parse_task_sort_by(value: Option<String>) -> Result<Option<TaskSortBy>, ErrorData> {
    value
        .map(|value| {
            value
                .parse::<TaskSortBy>()
                .map_err(|error| ErrorData::invalid_params(error, None))
        })
        .transpose()
}

pub(super) fn parse_sort_order(value: Option<String>) -> Result<Option<SortOrder>, ErrorData> {
    value
        .map(|value| {
            value
                .parse::<SortOrder>()
                .map_err(|error| ErrorData::invalid_params(error, None))
        })
        .transpose()
}
