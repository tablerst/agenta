CREATE TABLE IF NOT EXISTS sync_clients (
    client_id TEXT PRIMARY KEY NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS sync_conflicts (
    conflict_id TEXT PRIMARY KEY NOT NULL,
    remote_id TEXT NOT NULL,
    entity_kind TEXT NOT NULL,
    local_id TEXT NOT NULL,
    local_version INTEGER NOT NULL,
    remote_version INTEGER NOT NULL,
    local_mutation_id TEXT,
    remote_mutation_id INTEGER,
    conflict_kind TEXT NOT NULL,
    details_json TEXT NOT NULL,
    detected_at TEXT NOT NULL,
    resolved_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_sync_conflicts_unresolved
    ON sync_conflicts(remote_id, resolved_at, detected_at DESC);

CREATE INDEX IF NOT EXISTS idx_sync_conflicts_entity
    ON sync_conflicts(remote_id, entity_kind, local_id, resolved_at);

