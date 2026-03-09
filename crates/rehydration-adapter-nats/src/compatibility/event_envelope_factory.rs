use std::collections::BTreeMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::to_value;

use crate::NatsConsumerError;
use crate::compatibility::event_envelope::EventEnvelope;

static CORRELATION_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(crate) fn create_event_envelope<T>(
    event_type: &str,
    payload: &T,
    producer: &str,
    entity_id: &str,
    operation: Option<&str>,
) -> Result<EventEnvelope, NatsConsumerError>
where
    T: Serialize,
{
    let payload = to_value(payload).map_err(|error| {
        NatsConsumerError::Publish(format!("failed to serialize envelope payload: {error}"))
    })?;

    let envelope = EventEnvelope {
        event_type: event_type.to_string(),
        payload,
        idempotency_key: generate_idempotency_key(event_type, entity_id, operation)?,
        correlation_id: generate_correlation_id(),
        timestamp: current_iso8601_timestamp(),
        producer: producer.to_string(),
        causation_id: None,
        metadata: BTreeMap::new(),
    };
    envelope.validate()?;
    Ok(envelope)
}

fn generate_idempotency_key(
    event_type: &str,
    entity_id: &str,
    operation: Option<&str>,
) -> Result<String, NatsConsumerError> {
    if event_type.trim().is_empty() {
        return Err(NatsConsumerError::Publish(
            "event_type cannot be empty".to_string(),
        ));
    }
    if entity_id.trim().is_empty() {
        return Err(NatsConsumerError::Publish(
            "entity_id cannot be empty".to_string(),
        ));
    }

    let mut hasher = DefaultHasher::new();
    event_type.hash(&mut hasher);
    entity_id.hash(&mut hasher);
    if let Some(operation) = operation.filter(|value| !value.trim().is_empty()) {
        operation.hash(&mut hasher);
    }

    Ok(format!("{:016x}", hasher.finish()))
}

fn generate_correlation_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let counter = CORRELATION_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("corr-{nanos:x}-{counter:x}")
}

fn current_iso8601_timestamp() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let hours = (seconds / 3600) % 24;
    let minutes = (seconds / 60) % 60;
    let secs = seconds % 60;

    format!("1970-01-01T{hours:02}:{minutes:02}:{secs:02}+00:00")
}
