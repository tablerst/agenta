CREATE TABLE IF NOT EXISTS projects (
    project_id TEXT PRIMARY KEY NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    description TEXT,
    status TEXT NOT NULL,
    default_version_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS versions (
    version_id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES projects(project_id) ON DELETE CASCADE ON UPDATE CASCADE
);

CREATE TABLE IF NOT EXISTS tasks (
    task_id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL,
    version_id TEXT,
    title TEXT NOT NULL,
    summary TEXT,
    description TEXT,
    task_search_summary TEXT NOT NULL,
    task_context_digest TEXT NOT NULL,
    status TEXT NOT NULL,
    priority TEXT NOT NULL,
    created_by TEXT NOT NULL,
    updated_by TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    closed_at TEXT,
    FOREIGN KEY (project_id) REFERENCES projects(project_id) ON DELETE CASCADE ON UPDATE CASCADE,
    FOREIGN KEY (version_id) REFERENCES versions(version_id) ON DELETE SET NULL ON UPDATE CASCADE
);

CREATE TABLE IF NOT EXISTS task_activities (
    activity_id TEXT PRIMARY KEY NOT NULL,
    task_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    content TEXT NOT NULL,
    activity_search_summary TEXT NOT NULL,
    created_by TEXT NOT NULL,
    created_at TEXT NOT NULL,
    metadata_json TEXT NOT NULL,
    FOREIGN KEY (task_id) REFERENCES tasks(task_id) ON DELETE CASCADE ON UPDATE CASCADE
);

CREATE TABLE IF NOT EXISTS attachments (
    attachment_id TEXT PRIMARY KEY NOT NULL,
    task_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    mime TEXT NOT NULL,
    original_filename TEXT NOT NULL,
    original_path TEXT NOT NULL,
    storage_path TEXT NOT NULL,
    sha256 TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    summary TEXT NOT NULL,
    created_by TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (task_id) REFERENCES tasks(task_id) ON DELETE CASCADE ON UPDATE CASCADE
);

CREATE TABLE IF NOT EXISTS approval_requests (
    request_id TEXT PRIMARY KEY NOT NULL,
    action TEXT NOT NULL,
    requested_via TEXT NOT NULL,
    resource_ref TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    request_summary TEXT NOT NULL,
    requested_at TEXT NOT NULL,
    requested_by TEXT NOT NULL,
    reviewed_at TEXT,
    reviewed_by TEXT,
    review_note TEXT,
    result_json TEXT,
    error_json TEXT,
    status TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_versions_project_id ON versions(project_id);
CREATE INDEX IF NOT EXISTS idx_tasks_project_id ON tasks(project_id);
CREATE INDEX IF NOT EXISTS idx_tasks_version_id ON tasks(version_id);
CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
CREATE INDEX IF NOT EXISTS idx_task_activities_task_id ON task_activities(task_id);
CREATE INDEX IF NOT EXISTS idx_attachments_task_id ON attachments(task_id);
CREATE INDEX IF NOT EXISTS idx_approval_requests_status_requested_at
    ON approval_requests(status, requested_at DESC);

CREATE VIRTUAL TABLE IF NOT EXISTS tasks_fts USING fts5(
    title,
    task_search_summary,
    content = 'tasks',
    content_rowid = 'rowid'
);

CREATE VIRTUAL TABLE IF NOT EXISTS task_activities_fts USING fts5(
    activity_search_summary,
    content = 'task_activities',
    content_rowid = 'rowid'
);

CREATE TRIGGER IF NOT EXISTS tasks_ai AFTER INSERT ON tasks BEGIN
    INSERT INTO tasks_fts(rowid, title, task_search_summary)
    VALUES (new.rowid, new.title, new.task_search_summary);
END;

CREATE TRIGGER IF NOT EXISTS tasks_ad AFTER DELETE ON tasks BEGIN
    INSERT INTO tasks_fts(tasks_fts, rowid, title, task_search_summary)
    VALUES ('delete', old.rowid, old.title, old.task_search_summary);
END;

CREATE TRIGGER IF NOT EXISTS tasks_au AFTER UPDATE ON tasks BEGIN
    INSERT INTO tasks_fts(tasks_fts, rowid, title, task_search_summary)
    VALUES ('delete', old.rowid, old.title, old.task_search_summary);
    INSERT INTO tasks_fts(rowid, title, task_search_summary)
    VALUES (new.rowid, new.title, new.task_search_summary);
END;

CREATE TRIGGER IF NOT EXISTS task_activities_ai AFTER INSERT ON task_activities BEGIN
    INSERT INTO task_activities_fts(rowid, activity_search_summary)
    VALUES (new.rowid, new.activity_search_summary);
END;

CREATE TRIGGER IF NOT EXISTS task_activities_ad AFTER DELETE ON task_activities BEGIN
    INSERT INTO task_activities_fts(task_activities_fts, rowid, activity_search_summary)
    VALUES ('delete', old.rowid, old.activity_search_summary);
END;

CREATE TRIGGER IF NOT EXISTS task_activities_au AFTER UPDATE ON task_activities BEGIN
    INSERT INTO task_activities_fts(task_activities_fts, rowid, activity_search_summary)
    VALUES ('delete', old.rowid, old.activity_search_summary);
    INSERT INTO task_activities_fts(rowid, activity_search_summary)
    VALUES (new.rowid, new.activity_search_summary);
END;

INSERT INTO tasks_fts(tasks_fts) VALUES ('rebuild');
INSERT INTO task_activities_fts(task_activities_fts) VALUES ('rebuild');
