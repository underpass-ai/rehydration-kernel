use rehydration_application::{UpdateContextChange, UpdateContextCommand};
use rehydration_proto::fleet_context_v1::UpdateContextRequest;

pub(crate) fn map_update_context_command(request: UpdateContextRequest) -> UpdateContextCommand {
    UpdateContextCommand {
        root_node_id: request.story_id,
        role: request.role,
        work_item_id: request.task_id,
        changes: request
            .changes
            .into_iter()
            .map(|change| UpdateContextChange {
                operation: change.operation,
                entity_kind: change.entity_type,
                entity_id: change.entity_id,
                payload_json: change.payload,
                reason: change.reason,
                scopes: Vec::new(),
            })
            .collect(),
        expected_revision: None,
        expected_content_hash: None,
        idempotency_key: None,
        requested_by: None,
        persist_snapshot: false,
    }
}

#[cfg(test)]
mod tests {
    use rehydration_proto::fleet_context_v1::{ContextChange, UpdateContextRequest};

    use super::map_update_context_command;

    #[test]
    fn update_context_command_maps_legacy_external_fields() {
        let command = map_update_context_command(UpdateContextRequest {
            story_id: "story-123".to_string(),
            task_id: "task-7".to_string(),
            role: "DEV".to_string(),
            changes: vec![ContextChange {
                operation: "UPDATE".to_string(),
                entity_type: "decision".to_string(),
                entity_id: "decision-9".to_string(),
                payload: "{\"status\":\"accepted\"}".to_string(),
                reason: "refined".to_string(),
            }],
            timestamp: "2026-03-08T10:00:00Z".to_string(),
        });

        assert_eq!(command.root_node_id, "story-123");
        assert_eq!(command.work_item_id, "task-7");
        assert_eq!(command.changes[0].entity_kind, "decision");
        assert!(!command.persist_snapshot);
    }
}
