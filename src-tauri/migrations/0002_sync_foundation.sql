CREATE TABLE IF NOT EXISTS sync_entities (
    entity_kind TEXT NOT NULL,
    local_id TEXT NOT NULL,
    remote_id TEXT NOT NULL,
    remote_entity_id TEXT,
    local_version INTEGER NOT NULL,
    dirty INTEGER NOT NULL,
    last_synced_at TEXT,
    last_enqueued_mutation_id TEXT,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (entity_kind, local_id)
);

CREATE TABLE IF NOT EXISTS sync_outbox (
    mutation_id TEXT PRIMARY KEY NOT NULL,
    remote_id TEXT NOT NULL,
    entity_kind TEXT NOT NULL,
    local_id TEXT NOT NULL,
    operation TEXT NOT NULL,
    local_version INTEGER NOT NULL,
    payload_json TEXT NOT NULL,
    status TEXT NOT NULL,
    attempt_count INTEGER NOT NULL,
    last_attempt_at TEXT,
    acked_at TEXT,
    last_error TEXT,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS sync_checkpoints (
    remote_id TEXT NOT NULL,
    checkpoint_kind TEXT NOT NULL,
    checkpoint_value TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (remote_id, checkpoint_kind)
);

CREATE TABLE IF NOT EXISTS sync_tombstones (
    entity_kind TEXT NOT NULL,
    local_id TEXT NOT NULL,
    remote_id TEXT NOT NULL,
    remote_entity_id TEXT,
    deleted_at TEXT NOT NULL,
    purge_after TEXT NOT NULL,
    PRIMARY KEY (entity_kind, local_id, remote_id)
);

CREATE INDEX IF NOT EXISTS idx_sync_entities_remote_dirty
    ON sync_entities(remote_id, dirty, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_sync_outbox_remote_status_created
    ON sync_outbox(remote_id, status, created_at ASC);

CREATE INDEX IF NOT EXISTS idx_sync_outbox_entity_created
    ON sync_outbox(entity_kind, local_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_sync_tombstones_remote_deleted
    ON sync_tombstones(remote_id, deleted_at DESC);
