use super::*;
use crate::domain::{SyncEntityState, SyncOutboxEntry};

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

pub(super) fn remote_mutation_is_self_echo(
    remote_id: &str,
    client_id: Uuid,
    existing: &SyncEntityState,
    mutation: &RemoteMutation,
    local_outbox_entry: Option<&SyncOutboxEntry>,
) -> bool {
    let same_origin = mutation.origin_client_id == Some(client_id)
        && mutation.origin_mutation_id == existing.last_enqueued_mutation_id;
    if same_origin {
        return true;
    }

    let originless_legacy_mutation =
        mutation.origin_client_id.is_none() && mutation.origin_mutation_id.is_none();
    if !originless_legacy_mutation {
        return false;
    }

    let Some(local_outbox_entry) = local_outbox_entry else {
        return false;
    };

    local_outbox_entry.remote_id == remote_id
        && local_outbox_entry.entity_kind == mutation.entity_kind
        && local_outbox_entry.local_id == mutation.local_id
        && local_outbox_entry.operation == mutation.operation
        && local_outbox_entry.local_version == mutation.local_version
        && local_outbox_entry.status == SyncOutboxStatus::Acked
        && local_outbox_entry.payload_json == mutation.payload_json
}

#[cfg(test)]
mod tests {
    use super::{compare_remote_mutations_for_apply, remote_mutation_is_self_echo};
    use crate::domain::{
        SyncEntityKind, SyncEntityState, SyncOperation, SyncOutboxEntry, SyncOutboxStatus,
    };
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
                base_local_version: 0,
                origin_client_id: None,
                origin_mutation_id: None,
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
                base_local_version: 0,
                origin_client_id: None,
                origin_mutation_id: None,
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
                base_local_version: 0,
                origin_client_id: None,
                origin_mutation_id: None,
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

    #[test]
    fn self_echo_accepts_matching_origin() {
        let now = OffsetDateTime::now_utc();
        let client_id = Uuid::new_v4();
        let mutation_id = Uuid::new_v4();
        let task_id = Uuid::new_v4();
        let existing = sync_entity_state(task_id, Some(mutation_id), now);
        let mutation = remote_mutation(
            task_id,
            SyncOperation::Update,
            2,
            Some(client_id),
            Some(mutation_id),
            json!({"summary": "remote"}),
            now,
        );

        assert!(remote_mutation_is_self_echo(
            "primary", client_id, &existing, &mutation, None
        ));
    }

    #[test]
    fn self_echo_accepts_originless_legacy_payload_match() {
        let now = OffsetDateTime::now_utc();
        let client_id = Uuid::new_v4();
        let mutation_id = Uuid::new_v4();
        let task_id = Uuid::new_v4();
        let payload = json!({"summary": "same"});
        let existing = sync_entity_state(task_id, Some(mutation_id), now);
        let mutation = remote_mutation(
            task_id,
            SyncOperation::Update,
            2,
            None,
            None,
            payload.clone(),
            now,
        );
        let local_outbox_entry = sync_outbox_entry(
            mutation_id,
            task_id,
            SyncOperation::Update,
            2,
            SyncOutboxStatus::Acked,
            payload,
            now,
        );

        assert!(remote_mutation_is_self_echo(
            "primary",
            client_id,
            &existing,
            &mutation,
            Some(&local_outbox_entry)
        ));
    }

    #[test]
    fn self_echo_rejects_originless_payload_mismatch() {
        let now = OffsetDateTime::now_utc();
        let client_id = Uuid::new_v4();
        let mutation_id = Uuid::new_v4();
        let task_id = Uuid::new_v4();
        let existing = sync_entity_state(task_id, Some(mutation_id), now);
        let mutation = remote_mutation(
            task_id,
            SyncOperation::Update,
            2,
            None,
            None,
            json!({"summary": "remote"}),
            now,
        );
        let local_outbox_entry = sync_outbox_entry(
            mutation_id,
            task_id,
            SyncOperation::Update,
            2,
            SyncOutboxStatus::Acked,
            json!({"summary": "local"}),
            now,
        );

        assert!(!remote_mutation_is_self_echo(
            "primary",
            client_id,
            &existing,
            &mutation,
            Some(&local_outbox_entry)
        ));
    }

    fn sync_entity_state(
        local_id: Uuid,
        last_enqueued_mutation_id: Option<Uuid>,
        updated_at: OffsetDateTime,
    ) -> SyncEntityState {
        SyncEntityState {
            entity_kind: SyncEntityKind::Task,
            local_id,
            remote_id: "primary".to_string(),
            remote_entity_id: Some(local_id.to_string()),
            local_version: 2,
            dirty: false,
            last_synced_at: Some(updated_at),
            last_enqueued_mutation_id,
            updated_at,
        }
    }

    fn sync_outbox_entry(
        mutation_id: Uuid,
        local_id: Uuid,
        operation: SyncOperation,
        local_version: i64,
        status: SyncOutboxStatus,
        payload_json: serde_json::Value,
        created_at: OffsetDateTime,
    ) -> SyncOutboxEntry {
        SyncOutboxEntry {
            mutation_id,
            remote_id: "primary".to_string(),
            entity_kind: SyncEntityKind::Task,
            local_id,
            operation,
            local_version,
            payload_json,
            status,
            attempt_count: 1,
            last_attempt_at: Some(created_at),
            acked_at: Some(created_at),
            last_error: None,
            created_at,
        }
    }

    fn remote_mutation(
        local_id: Uuid,
        operation: SyncOperation,
        local_version: i64,
        origin_client_id: Option<Uuid>,
        origin_mutation_id: Option<Uuid>,
        payload_json: serde_json::Value,
        created_at: OffsetDateTime,
    ) -> RemoteMutation {
        RemoteMutation {
            remote_mutation_id: 42,
            entity_kind: SyncEntityKind::Task,
            remote_entity_id: local_id.to_string(),
            local_id,
            operation,
            local_version,
            base_local_version: local_version.saturating_sub(1),
            origin_client_id,
            origin_mutation_id,
            payload_json,
            created_at,
            attachment_blob: None,
        }
    }
}
