CREATE TABLE IF NOT EXISTS search_index_jobs (
    task_id TEXT PRIMARY KEY,
    job_kind TEXT NOT NULL,
    status TEXT NOT NULL,
    attempt_count INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    next_attempt_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_search_index_jobs_status_due
    ON search_index_jobs(status, next_attempt_at, updated_at);
