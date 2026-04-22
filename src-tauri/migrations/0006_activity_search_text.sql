ALTER TABLE task_activities
    ADD COLUMN activity_search_text TEXT NOT NULL DEFAULT '';

UPDATE task_activities
SET activity_search_text = CASE
    WHEN trim(content) = '' THEN activity_search_summary
    ELSE content
END
WHERE trim(activity_search_text) = '';

DROP TABLE IF EXISTS task_activities_fts;
DROP TRIGGER IF EXISTS task_activities_ai;
DROP TRIGGER IF EXISTS task_activities_ad;
DROP TRIGGER IF EXISTS task_activities_au;

CREATE VIRTUAL TABLE IF NOT EXISTS task_activities_fts USING fts5(
    activity_search_summary,
    activity_search_text,
    content = 'task_activities',
    content_rowid = 'rowid'
);

CREATE TRIGGER IF NOT EXISTS task_activities_ai AFTER INSERT ON task_activities BEGIN
    INSERT INTO task_activities_fts(rowid, activity_search_summary, activity_search_text)
    VALUES (new.rowid, new.activity_search_summary, new.activity_search_text);
END;

CREATE TRIGGER IF NOT EXISTS task_activities_ad AFTER DELETE ON task_activities BEGIN
    INSERT INTO task_activities_fts(task_activities_fts, rowid, activity_search_summary, activity_search_text)
    VALUES ('delete', old.rowid, old.activity_search_summary, old.activity_search_text);
END;

CREATE TRIGGER IF NOT EXISTS task_activities_au AFTER UPDATE ON task_activities BEGIN
    INSERT INTO task_activities_fts(task_activities_fts, rowid, activity_search_summary, activity_search_text)
    VALUES ('delete', old.rowid, old.activity_search_summary, old.activity_search_text);
    INSERT INTO task_activities_fts(rowid, activity_search_summary, activity_search_text)
    VALUES (new.rowid, new.activity_search_summary, new.activity_search_text);
END;

INSERT INTO task_activities_fts(task_activities_fts) VALUES ('rebuild');
