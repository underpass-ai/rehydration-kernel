use rehydration_application::{UpdateContextChange, UpdateContextCommand};

use crate::NatsConsumerError;
use crate::compatibility::update_context_request_payload::UpdateContextRequestPayload;

pub(crate) fn map_update_context_command(
    payload: UpdateContextRequestPayload,
) -> Result<UpdateContextCommand, NatsConsumerError> {
    if payload.story_id.trim().is_empty() {
        return Err(NatsConsumerError::InvalidRequest(
            "story_id is required in update context request".to_string(),
        ));
    }

    Ok(UpdateContextCommand {
        root_node_id: payload.story_id,
        role: payload.role,
        work_item_id: payload.task_id,
        changes: payload
            .changes
            .into_iter()
            .map(|change| UpdateContextChange {
                operation: change.operation,
                entity_kind: change.entity_type,
                entity_id: change.entity_id,
                payload_json: serialize_payload(change.payload),
                reason: change.reason,
                scopes: Vec::new(),
            })
            .collect(),
        expected_revision: None,
        expected_content_hash: None,
        idempotency_key: None,
        requested_by: None,
        persist_snapshot: false,
    })
}

fn serialize_payload(payload: Option<serde_json::Value>) -> String {
    match payload {
        None | Some(serde_json::Value::Null) => String::new(),
        Some(serde_json::Value::String(value)) => value,
        Some(serde_json::Value::Object(value)) => serde_json::Value::Object(value).to_string(),
        Some(other) => other.to_string(),
    }
}
