CREATE TABLE IF NOT EXISTS search_index_runs (
    run_id TEXT PRIMARY KEY NOT NULL,
    status TEXT NOT NULL,
    trigger_kind TEXT NOT NULL,
    scanned INTEGER NOT NULL,
    queued INTEGER NOT NULL,
    skipped INTEGER NOT NULL,
    processed INTEGER NOT NULL,
    succeeded INTEGER NOT NULL,
    failed INTEGER NOT NULL,
    batch_size INTEGER NOT NULL,
    started_at TEXT NOT NULL,
    finished_at TEXT,
    last_error TEXT,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_search_index_runs_updated
    ON search_index_runs(updated_at DESC);

ALTER TABLE search_index_jobs ADD COLUMN run_id TEXT;
ALTER TABLE search_index_jobs ADD COLUMN locked_at TEXT;
ALTER TABLE search_index_jobs ADD COLUMN lease_until TEXT;

CREATE INDEX IF NOT EXISTS idx_search_index_jobs_run
    ON search_index_jobs(run_id, status, updated_at);
