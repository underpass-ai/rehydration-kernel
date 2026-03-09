use serde_json::Value;

use crate::NatsConsumerError;
use crate::compatibility::event_envelope::EventEnvelope;

pub(crate) fn parse_required_envelope(payload: &[u8]) -> Result<EventEnvelope, NatsConsumerError> {
    let data = serde_json::from_slice::<Value>(payload)
        .map_err(|error| NatsConsumerError::InvalidPayload(error.to_string()))?;
    let object = data.as_object().ok_or_else(|| {
        NatsConsumerError::InvalidEnvelope(format!(
            "EventEnvelope payload must be a dict, got {}",
            json_type_name(&data)
        ))
    })?;

    let event_type = required_string(object, "event_type")?;
    let payload = object
        .get("payload")
        .cloned()
        .ok_or_else(|| missing_required_field("payload"))?;
    let idempotency_key = required_string(object, "idempotency_key")?;
    let correlation_id = required_string(object, "correlation_id")?;
    let timestamp = required_string(object, "timestamp")?;
    let producer = required_string(object, "producer")?;
    let causation_id = optional_string(object, "causation_id")?;
    let metadata = optional_metadata(object, "metadata")?;

    let envelope = EventEnvelope {
        event_type,
        payload,
        idempotency_key,
        correlation_id,
        timestamp,
        producer,
        causation_id,
        metadata,
    };
    envelope.validate()?;
    let _ = envelope.payload_object()?;

    Ok(envelope)
}

fn required_string(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<String, NatsConsumerError> {
    let value = object
        .get(field)
        .ok_or_else(|| missing_required_field(field))?;

    value
        .as_str()
        .map(|value| value.to_string())
        .ok_or_else(|| {
            NatsConsumerError::InvalidEnvelope(format!(
                "EventEnvelope field {field} must be a string"
            ))
        })
}

fn optional_string(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<Option<String>, NatsConsumerError> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_str()
            .map(|value| Some(value.to_string()))
            .ok_or_else(|| {
                NatsConsumerError::InvalidEnvelope(format!(
                    "EventEnvelope field {field} must be a string"
                ))
            }),
    }
}

fn optional_metadata(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<std::collections::BTreeMap<String, Value>, NatsConsumerError> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(std::collections::BTreeMap::new()),
        Some(Value::Object(metadata)) => Ok(metadata
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect()),
        Some(_) => Err(NatsConsumerError::InvalidEnvelope(format!(
            "EventEnvelope field {field} must be an object"
        ))),
    }
}

fn missing_required_field(field: &str) -> NatsConsumerError {
    NatsConsumerError::InvalidEnvelope(format!("Missing required EventEnvelope field: '{field}'"))
}

fn json_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::parse_required_envelope;

    #[test]
    fn parser_rejects_missing_required_field() {
        let error = parse_required_envelope(
            json!({
                "payload": {"story_id": "story-1"},
                "idempotency_key": "idemp-1",
                "correlation_id": "corr-1",
                "timestamp": "2026-03-09T19:30:00+00:00",
                "producer": "context-tests",
                "metadata": {}
            })
            .to_string()
            .as_bytes(),
        )
        .expect_err("missing event_type must fail");

        assert!(matches!(
            error,
            crate::NatsConsumerError::InvalidEnvelope(_)
        ));
    }

    #[test]
    fn parser_rejects_invalid_metadata_shape() {
        let error = parse_required_envelope(
            json!({
                "event_type": "context.update.request",
                "payload": {"story_id": "story-1"},
                "idempotency_key": "idemp-1",
                "correlation_id": "corr-1",
                "timestamp": "2026-03-09T19:30:00+00:00",
                "producer": "context-tests",
                "metadata": "bad"
            })
            .to_string()
            .as_bytes(),
        )
        .expect_err("invalid metadata must fail");

        assert!(matches!(
            error,
            crate::NatsConsumerError::InvalidEnvelope(_)
        ));
    }
}
