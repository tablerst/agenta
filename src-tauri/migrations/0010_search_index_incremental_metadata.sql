CREATE TABLE IF NOT EXISTS search_index_embedding_profiles (
    profile_id TEXT PRIMARY KEY NOT NULL,
    provider TEXT NOT NULL,
    base_url TEXT NOT NULL,
    model TEXT NOT NULL,
    fingerprint TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS search_index_documents (
    vector_id TEXT PRIMARY KEY NOT NULL,
    task_id TEXT NOT NULL,
    source_kind TEXT NOT NULL,
    document_hash TEXT NOT NULL,
    embedding_fingerprint TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_search_index_documents_task
    ON search_index_documents(task_id, updated_at);

CREATE INDEX IF NOT EXISTS idx_search_index_documents_fingerprint
    ON search_index_documents(embedding_fingerprint, updated_at);

ALTER TABLE search_index_runs ADD COLUMN unchanged INTEGER NOT NULL DEFAULT 0;
ALTER TABLE search_index_runs ADD COLUMN embedding_fingerprint TEXT;
