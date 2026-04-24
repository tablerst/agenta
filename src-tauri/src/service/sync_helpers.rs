use super::*;

pub(super) fn sync_entity_dependency_rank(entity_kind: SyncEntityKind) -> u8 {
    match entity_kind {
        SyncEntityKind::Project => 0,
        SyncEntityKind::Version => 1,
        SyncEntityKind::Task => 2,
        SyncEntityKind::TaskRelation => 3,
        SyncEntityKind::Note => 4,
        SyncEntityKind::Attachment => 5,
    }
}

pub(super) fn compare_remote_mutations_for_apply(
    left: &RemoteMutation,
    right: &RemoteMutation,
) -> std::cmp::Ordering {
    sync_entity_dependency_rank(left.entity_kind)
        .cmp(&sync_entity_dependency_rank(right.entity_kind))
        .then(left.remote_mutation_id.cmp(&right.remote_mutation_id))
}

#[cfg(test)]
mod tests {
    use super::compare_remote_mutations_for_apply;
    use crate::domain::{SyncEntityKind, SyncOperation};
    use crate::sync::RemoteMutation;
    use serde_json::json;
    use time::OffsetDateTime;
    use uuid::Uuid;

    #[test]
    fn remote_mutation_apply_order_prioritizes_dependencies() {
        let now = OffsetDateTime::now_utc();
        let mut mutations = vec![
            RemoteMutation {
                remote_mutation_id: 9,
                entity_kind: SyncEntityKind::TaskRelation,
                remote_entity_id: "relation".to_string(),
                local_id: Uuid::new_v4(),
                operation: SyncOperation::Create,
                local_version: 1,
                payload_json: json!({}),
                created_at: now,
                attachment_blob: None,
            },
            RemoteMutation {
                remote_mutation_id: 10,
                entity_kind: SyncEntityKind::Task,
                remote_entity_id: "task".to_string(),
                local_id: Uuid::new_v4(),
                operation: SyncOperation::Create,
                local_version: 1,
                payload_json: json!({}),
                created_at: now,
                attachment_blob: None,
            },
            RemoteMutation {
                remote_mutation_id: 8,
                entity_kind: SyncEntityKind::Project,
                remote_entity_id: "project".to_string(),
                local_id: Uuid::new_v4(),
                operation: SyncOperation::Create,
                local_version: 1,
                payload_json: json!({}),
                created_at: now,
                attachment_blob: None,
            },
        ];

        mutations.sort_by(compare_remote_mutations_for_apply);

        assert_eq!(mutations[0].entity_kind, SyncEntityKind::Project);
        assert_eq!(mutations[1].entity_kind, SyncEntityKind::Task);
        assert_eq!(mutations[2].entity_kind, SyncEntityKind::TaskRelation);
    }
}
