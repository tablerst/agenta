ALTER TABLE tasks ADD COLUMN task_code TEXT;
ALTER TABLE tasks ADD COLUMN task_kind TEXT NOT NULL DEFAULT 'standard';
ALTER TABLE tasks ADD COLUMN latest_note_summary TEXT;
ALTER TABLE tasks ADD COLUMN knowledge_status TEXT NOT NULL DEFAULT 'empty';

CREATE INDEX IF NOT EXISTS idx_tasks_task_code ON tasks(task_code);
CREATE INDEX IF NOT EXISTS idx_tasks_task_kind ON tasks(task_kind);
CREATE INDEX IF NOT EXISTS idx_tasks_knowledge_status ON tasks(knowledge_status);

UPDATE tasks
SET task_code = (
    CASE
        WHEN instr(trim(title), ' ') > 0 THEN substr(trim(title), 1, instr(trim(title), ' ') - 1)
        ELSE trim(title)
    END
)
WHERE task_code IS NULL
  AND (
      CASE
          WHEN instr(trim(title), ' ') > 0 THEN substr(trim(title), 1, instr(trim(title), ' ') - 1)
          ELSE trim(title)
      END
  ) GLOB '*-[0-9]*';

UPDATE tasks
SET task_kind = 'index'
WHERE task_kind = 'standard'
  AND (
      lower(title) LIKE '%index%'
      OR lower(COALESCE(summary, '')) LIKE '%index%'
      OR title LIKE '%索引%'
      OR title LIKE '%汇总%'
      OR title LIKE '%导航%'
      OR COALESCE(summary, '') LIKE '%索引%'
      OR COALESCE(summary, '') LIKE '%汇总%'
      OR COALESCE(summary, '') LIKE '%导航%'
  );

UPDATE tasks
SET task_kind = 'context'
WHERE task_kind = 'standard'
  AND (
      COALESCE(task_code, '') LIKE 'InitCtx-%'
      OR lower(title) LIKE '%context%'
      OR lower(COALESCE(summary, '')) LIKE '%context%'
      OR title LIKE '%上下文%'
      OR title LIKE '%模块%'
      OR COALESCE(summary, '') LIKE '%上下文%'
      OR COALESCE(summary, '') LIKE '%模块%'
  );

UPDATE tasks
SET latest_note_summary = (
    SELECT ta.activity_search_summary
    FROM task_activities ta
    WHERE ta.task_id = tasks.task_id
      AND ta.kind = 'note'
    ORDER BY ta.created_at DESC, ta.activity_id DESC
    LIMIT 1
);

UPDATE tasks
SET knowledge_status = CASE
    WHEN EXISTS (
        SELECT 1
        FROM task_activities ta
        WHERE ta.task_id = tasks.task_id
          AND ta.kind = 'note'
          AND json_extract(ta.metadata_json, '$.note_kind') = 'conclusion'
    ) THEN 'reusable'
    WHEN EXISTS (
        SELECT 1
        FROM task_activities ta
        WHERE ta.task_id = tasks.task_id
          AND ta.kind = 'note'
    ) THEN 'working'
    ELSE 'empty'
END;

UPDATE tasks
SET task_context_digest = substr(
    'status=' || status
    || ' priority=' || priority
    || ' task_code=' || COALESCE(task_code, '')
    || ' task_kind=' || task_kind
    || ' knowledge_status=' || knowledge_status
    || ' latest_note_summary=' || COALESCE(latest_note_summary, '')
    || ' title=' || title
    || ' summary=' || COALESCE(summary, '')
    || ' description=' || COALESCE(description, ''),
    1,
    320
);

DROP TRIGGER IF EXISTS tasks_ai;
DROP TRIGGER IF EXISTS tasks_ad;
DROP TRIGGER IF EXISTS tasks_au;
DROP TABLE IF EXISTS tasks_fts;

CREATE VIRTUAL TABLE IF NOT EXISTS tasks_fts USING fts5(
    title,
    task_code,
    task_kind,
    task_search_summary,
    task_context_digest,
    latest_note_summary,
    content = 'tasks',
    content_rowid = 'rowid'
);

CREATE TRIGGER IF NOT EXISTS tasks_ai AFTER INSERT ON tasks BEGIN
    INSERT INTO tasks_fts(rowid, title, task_code, task_kind, task_search_summary, task_context_digest, latest_note_summary)
    VALUES (new.rowid, new.title, new.task_code, new.task_kind, new.task_search_summary, new.task_context_digest, new.latest_note_summary);
END;

CREATE TRIGGER IF NOT EXISTS tasks_ad AFTER DELETE ON tasks BEGIN
    INSERT INTO tasks_fts(tasks_fts, rowid, title, task_code, task_kind, task_search_summary, task_context_digest, latest_note_summary)
    VALUES ('delete', old.rowid, old.title, old.task_code, old.task_kind, old.task_search_summary, old.task_context_digest, old.latest_note_summary);
END;

CREATE TRIGGER IF NOT EXISTS tasks_au AFTER UPDATE ON tasks BEGIN
    INSERT INTO tasks_fts(tasks_fts, rowid, title, task_code, task_kind, task_search_summary, task_context_digest, latest_note_summary)
    VALUES ('delete', old.rowid, old.title, old.task_code, old.task_kind, old.task_search_summary, old.task_context_digest, old.latest_note_summary);
    INSERT INTO tasks_fts(rowid, title, task_code, task_kind, task_search_summary, task_context_digest, latest_note_summary)
    VALUES (new.rowid, new.title, new.task_code, new.task_kind, new.task_search_summary, new.task_context_digest, new.latest_note_summary);
END;

INSERT INTO tasks_fts(tasks_fts) VALUES ('rebuild');
