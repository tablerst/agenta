CREATE TABLE IF NOT EXISTS task_relations (
    relation_id TEXT PRIMARY KEY NOT NULL,
    kind TEXT NOT NULL,
    source_task_id TEXT NOT NULL,
    target_task_id TEXT NOT NULL,
    status TEXT NOT NULL,
    created_by TEXT NOT NULL,
    updated_by TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    resolved_at TEXT,
    CHECK (source_task_id <> target_task_id),
    FOREIGN KEY (source_task_id) REFERENCES tasks(task_id) ON DELETE CASCADE ON UPDATE CASCADE,
    FOREIGN KEY (target_task_id) REFERENCES tasks(task_id) ON DELETE CASCADE ON UPDATE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_task_relations_source_kind_status
    ON task_relations(source_task_id, kind, status, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_task_relations_target_kind_status
    ON task_relations(target_task_id, kind, status, updated_at DESC);

CREATE UNIQUE INDEX IF NOT EXISTS idx_task_relations_active_unique
    ON task_relations(kind, source_task_id, target_task_id)
    WHERE status = 'active';

CREATE UNIQUE INDEX IF NOT EXISTS idx_task_relations_active_parent
    ON task_relations(target_task_id)
    WHERE kind = 'parent_child' AND status = 'active';
