CREATE TABLE IF NOT EXISTS task_activity_chunks (
    chunk_id TEXT PRIMARY KEY NOT NULL,
    activity_id TEXT NOT NULL REFERENCES task_activities(activity_id) ON DELETE CASCADE,
    task_id TEXT NOT NULL,
    chunk_index INTEGER NOT NULL,
    chunk_text TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_task_activity_chunks_activity_id
    ON task_activity_chunks(activity_id);
CREATE INDEX IF NOT EXISTS idx_task_activity_chunks_task_id
    ON task_activity_chunks(task_id);

CREATE VIRTUAL TABLE IF NOT EXISTS task_activity_chunks_fts USING fts5(
    chunk_text,
    content = 'task_activity_chunks',
    content_rowid = 'rowid'
);

CREATE TRIGGER IF NOT EXISTS task_activity_chunks_ai AFTER INSERT ON task_activity_chunks BEGIN
    INSERT INTO task_activity_chunks_fts(rowid, chunk_text)
    VALUES (new.rowid, new.chunk_text);
END;

CREATE TRIGGER IF NOT EXISTS task_activity_chunks_ad AFTER DELETE ON task_activity_chunks BEGIN
    INSERT INTO task_activity_chunks_fts(task_activity_chunks_fts, rowid, chunk_text)
    VALUES ('delete', old.rowid, old.chunk_text);
END;

CREATE TRIGGER IF NOT EXISTS task_activity_chunks_au AFTER UPDATE ON task_activity_chunks BEGIN
    INSERT INTO task_activity_chunks_fts(task_activity_chunks_fts, rowid, chunk_text)
    VALUES ('delete', old.rowid, old.chunk_text);
    INSERT INTO task_activity_chunks_fts(rowid, chunk_text)
    VALUES (new.rowid, new.chunk_text);
END;

INSERT INTO task_activity_chunks_fts(task_activity_chunks_fts) VALUES ('rebuild');
