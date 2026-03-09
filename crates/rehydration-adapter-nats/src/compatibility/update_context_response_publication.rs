use crate::NatsConsumerError;
use crate::compatibility::event_envelope_factory::create_event_envelope;
use crate::compatibility::publication::NatsPublication;
use crate::compatibility::update_context_response_payload::UpdateContextResponsePayload;
use rehydration_application::UpdateContextOutcome;

pub(crate) fn build_update_context_response_publication(
    story_id: &str,
    outcome: &UpdateContextOutcome,
) -> Result<NatsPublication, NatsConsumerError> {
    let payload = UpdateContextResponsePayload::new(story_id.to_string(), outcome);
    let envelope = create_event_envelope(
        "context.update.response",
        &payload,
        "context-service",
        story_id,
        Some("update_response"),
    )?;
    let payload = serde_json::to_vec(&envelope).map_err(|error| {
        NatsConsumerError::Publish(format!(
            "failed to serialize update context response envelope: {error}"
        ))
    })?;

    Ok(NatsPublication {
        subject: "context.update.response".to_string(),
        payload,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use rehydration_application::{AcceptedVersion, UpdateContextOutcome};

    use super::build_update_context_response_publication;

    #[test]
    fn update_response_publication_uses_frozen_subject_and_event_type() {
        let publication = build_update_context_response_publication(
            "story-1",
            &UpdateContextOutcome {
                accepted_version: AcceptedVersion {
                    revision: 2,
                    content_hash: "hash-2".to_string(),
                    generator_version: "0.1.0".to_string(),
                },
                warnings: vec!["warn".to_string()],
                snapshot_persisted: false,
                snapshot_id: None,
            },
        )
        .expect("publication should serialize");

        assert_eq!(publication.subject, "context.update.response");
        let envelope: Value =
            serde_json::from_slice(&publication.payload).expect("payload must be json");
        assert_eq!(envelope["event_type"], "context.update.response");
        assert_eq!(envelope["payload"]["story_id"], "story-1");
        assert_eq!(envelope["payload"]["version"], 2);
    }
}
