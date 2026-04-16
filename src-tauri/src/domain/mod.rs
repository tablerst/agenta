use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

macro_rules! string_enum {
    ($name:ident { $($variant:ident => $value:literal),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
        #[serde(rename_all = "snake_case")]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            pub fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $value),+
                }
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl std::str::FromStr for $name {
            type Err = String;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                match value {
                    $($value => Ok(Self::$variant)),+,
                    other => Err(format!("invalid {} value: {}", stringify!($name), other)),
                }
            }
        }
    };
}

string_enum!(ProjectStatus {
    Active => "active",
    Archived => "archived",
});

string_enum!(VersionStatus {
    Planning => "planning",
    Active => "active",
    Closed => "closed",
    Archived => "archived",
});

string_enum!(TaskStatus {
    Draft => "draft",
    Ready => "ready",
    InProgress => "in_progress",
    Blocked => "blocked",
    Done => "done",
    Cancelled => "cancelled",
});

string_enum!(TaskPriority {
    Low => "low",
    Normal => "normal",
    High => "high",
    Critical => "critical",
});

string_enum!(TaskActivityKind {
    Note => "note",
    StatusChange => "status_change",
    System => "system",
    AttachmentRef => "attachment_ref",
});

string_enum!(TaskRelationKind {
    ParentChild => "parent_child",
    Blocks => "blocks",
});

string_enum!(TaskRelationStatus {
    Active => "active",
    Resolved => "resolved",
});

string_enum!(AttachmentKind {
    Screenshot => "screenshot",
    Image => "image",
    Log => "log",
    Report => "report",
    Patch => "patch",
    Artifact => "artifact",
    Other => "other",
});

string_enum!(ApprovalStatus {
    Pending => "pending",
    Approved => "approved",
    Denied => "denied",
    Failed => "failed",
});

string_enum!(ApprovalRequestedVia {
    Cli => "cli",
    Mcp => "mcp",
    Desktop => "desktop",
});

string_enum!(SyncMode {
    ManualBidirectional => "manual_bidirectional",
});

string_enum!(SyncEntityKind {
    Project => "project",
    Version => "version",
    Task => "task",
    TaskRelation => "task_relation",
    Note => "note",
    Attachment => "attachment",
});

string_enum!(SyncOperation {
    Create => "create",
    Update => "update",
});

string_enum!(SyncOutboxStatus {
    Pending => "pending",
    Acked => "acked",
    Failed => "failed",
});

string_enum!(SyncCheckpointKind {
    Pull => "pull",
    PushAck => "push_ack",
});

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    pub project_id: Uuid,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub status: ProjectStatus,
    pub default_version_id: Option<Uuid>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Version {
    pub version_id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub status: VersionStatus,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Task {
    pub task_id: Uuid,
    pub project_id: Uuid,
    pub version_id: Option<Uuid>,
    pub title: String,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub task_search_summary: String,
    pub task_context_digest: String,
    pub status: TaskStatus,
    pub priority: TaskPriority,
    pub created_by: String,
    pub updated_by: String,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub closed_at: Option<OffsetDateTime>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskActivity {
    pub activity_id: Uuid,
    pub task_id: Uuid,
    pub kind: TaskActivityKind,
    pub content: String,
    pub activity_search_summary: String,
    pub created_by: String,
    pub created_at: OffsetDateTime,
    pub metadata_json: serde_json::Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskRelation {
    pub relation_id: Uuid,
    pub kind: TaskRelationKind,
    pub source_task_id: Uuid,
    pub target_task_id: Uuid,
    pub status: TaskRelationStatus,
    pub created_by: String,
    pub updated_by: String,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub resolved_at: Option<OffsetDateTime>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Attachment {
    pub attachment_id: Uuid,
    pub task_id: Uuid,
    pub kind: AttachmentKind,
    pub mime: String,
    pub original_filename: String,
    pub original_path: String,
    pub storage_path: String,
    pub sha256: String,
    pub size_bytes: i64,
    pub summary: String,
    pub created_by: String,
    pub created_at: OffsetDateTime,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApprovalRequest {
    pub request_id: Uuid,
    pub action: String,
    pub requested_via: ApprovalRequestedVia,
    pub resource_ref: String,
    pub project_ref: Option<String>,
    pub project_name: Option<String>,
    pub task_ref: Option<String>,
    pub payload_json: serde_json::Value,
    pub request_summary: String,
    pub requested_at: OffsetDateTime,
    pub requested_by: String,
    pub reviewed_at: Option<OffsetDateTime>,
    pub reviewed_by: Option<String>,
    pub review_note: Option<String>,
    pub result_json: Option<serde_json::Value>,
    pub error_json: Option<serde_json::Value>,
    pub status: ApprovalStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncEntityState {
    pub entity_kind: SyncEntityKind,
    pub local_id: Uuid,
    pub remote_id: String,
    pub remote_entity_id: Option<String>,
    pub local_version: i64,
    pub dirty: bool,
    pub last_synced_at: Option<OffsetDateTime>,
    pub last_enqueued_mutation_id: Option<Uuid>,
    pub updated_at: OffsetDateTime,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncOutboxEntry {
    pub mutation_id: Uuid,
    pub remote_id: String,
    pub entity_kind: SyncEntityKind,
    pub local_id: Uuid,
    pub operation: SyncOperation,
    pub local_version: i64,
    pub payload_json: serde_json::Value,
    pub status: SyncOutboxStatus,
    pub attempt_count: i64,
    pub last_attempt_at: Option<OffsetDateTime>,
    pub acked_at: Option<OffsetDateTime>,
    pub last_error: Option<String>,
    pub created_at: OffsetDateTime,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncCheckpoint {
    pub remote_id: String,
    pub checkpoint_kind: SyncCheckpointKind,
    pub checkpoint_value: String,
    pub updated_at: OffsetDateTime,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncTombstone {
    pub entity_kind: SyncEntityKind,
    pub local_id: Uuid,
    pub remote_id: String,
    pub remote_entity_id: Option<String>,
    pub deleted_at: OffsetDateTime,
    pub purge_after: OffsetDateTime,
}

impl Default for ProjectStatus {
    fn default() -> Self {
        Self::Active
    }
}

impl Default for VersionStatus {
    fn default() -> Self {
        Self::Planning
    }
}

impl Default for TaskStatus {
    fn default() -> Self {
        Self::Ready
    }
}

impl Default for TaskPriority {
    fn default() -> Self {
        Self::Normal
    }
}

impl Default for AttachmentKind {
    fn default() -> Self {
        Self::Other
    }
}

impl Default for ApprovalStatus {
    fn default() -> Self {
        Self::Pending
    }
}

impl Default for SyncMode {
    fn default() -> Self {
        Self::ManualBidirectional
    }
}

impl Default for TaskRelationStatus {
    fn default() -> Self {
        Self::Active
    }
}

impl Default for SyncOutboxStatus {
    fn default() -> Self {
        Self::Pending
    }
}
