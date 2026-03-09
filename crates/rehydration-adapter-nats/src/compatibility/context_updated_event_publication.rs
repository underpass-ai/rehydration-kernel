use std::time::{SystemTime, UNIX_EPOCH};

use crate::NatsConsumerError;
use crate::compatibility::context_updated_event_payload::ContextUpdatedEventPayload;
use crate::compatibility::event_envelope_factory::create_event_envelope;
use crate::compatibility::publication::NatsPublication;

const CONTEXT_UPDATED_SUBJECT: &str = "context.events.updated";
const CONTEXT_UPDATED_EVENT_TYPE: &str = "context.updated";

pub(crate) fn build_context_updated_publication(
    story_id: &str,
    version: u64,
) -> Result<NatsPublication, NatsConsumerError> {
    let payload = ContextUpdatedEventPayload::new(story_id, version, current_timestamp_seconds());
    let envelope = create_event_envelope(
        CONTEXT_UPDATED_EVENT_TYPE,
        &payload,
        "context-service",
        story_id,
        Some("context_updated"),
    )?;
    let payload = serde_json::to_vec(&envelope).map_err(|error| {
        NatsConsumerError::Publish(format!(
            "failed to serialize context updated envelope: {error}"
        ))
    })?;

    Ok(NatsPublication {
        subject: CONTEXT_UPDATED_SUBJECT.to_string(),
        payload,
    })
}

fn current_timestamp_seconds() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::build_context_updated_publication;

    #[test]
    fn context_updated_publication_uses_frozen_subject_and_event_type() {
        let publication =
            build_context_updated_publication("story-1", 7).expect("publication should serialize");

        assert_eq!(publication.subject, "context.events.updated");
        let envelope: Value =
            serde_json::from_slice(&publication.payload).expect("payload must be json");
        assert_eq!(envelope["event_type"], "context.updated");
        assert_eq!(envelope["payload"]["story_id"], "story-1");
        assert_eq!(envelope["payload"]["version"], 7);
        assert!(envelope["payload"]["timestamp"].is_number());
    }
}
