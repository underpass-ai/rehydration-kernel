use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::NatsConsumerError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct EventEnvelope {
    pub event_type: String,
    pub payload: Value,
    pub idempotency_key: String,
    pub correlation_id: String,
    pub timestamp: String,
    pub producer: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub causation_id: Option<String>,
    #[serde(default)]
    pub metadata: BTreeMap<String, Value>,
}

impl EventEnvelope {
    pub(crate) fn validate(&self) -> Result<(), NatsConsumerError> {
        if self.event_type.trim().is_empty() {
            return Err(NatsConsumerError::InvalidEnvelope(
                "event_type cannot be empty".to_string(),
            ));
        }
        if self.idempotency_key.trim().is_empty() {
            return Err(NatsConsumerError::InvalidEnvelope(
                "idempotency_key cannot be empty".to_string(),
            ));
        }
        if self.correlation_id.trim().is_empty() {
            return Err(NatsConsumerError::InvalidEnvelope(
                "correlation_id cannot be empty".to_string(),
            ));
        }
        if self.timestamp.trim().is_empty() {
            return Err(NatsConsumerError::InvalidEnvelope(
                "timestamp cannot be empty".to_string(),
            ));
        }
        if !looks_like_iso8601(&self.timestamp) {
            return Err(NatsConsumerError::InvalidEnvelope(format!(
                "Invalid timestamp format (expected ISO 8601): {}",
                self.timestamp
            )));
        }
        if self.producer.trim().is_empty() {
            return Err(NatsConsumerError::InvalidEnvelope(
                "producer cannot be empty".to_string(),
            ));
        }
        Ok(())
    }

    pub(crate) fn payload_object(
        &self,
    ) -> Result<&serde_json::Map<String, Value>, NatsConsumerError> {
        self.payload.as_object().ok_or_else(|| {
            NatsConsumerError::InvalidEnvelope(format!(
                "payload must be an object, got {}",
                json_type_name(&self.payload)
            ))
        })
    }
}

fn looks_like_iso8601(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 20
        && bytes.get(4) == Some(&b'-')
        && bytes.get(7) == Some(&b'-')
        && bytes.get(10) == Some(&b'T')
        && bytes.get(13) == Some(&b':')
        && bytes.get(16) == Some(&b':')
        && (value.ends_with('Z') || value.ends_with("+00:00") || value.contains('+'))
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
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::EventEnvelope;

    #[test]
    fn validate_rejects_invalid_timestamp() {
        let envelope = EventEnvelope {
            event_type: "context.update.request".to_string(),
            payload: json!({"story_id": "story-1"}),
            idempotency_key: "idemp-1".to_string(),
            correlation_id: "corr-1".to_string(),
            timestamp: "invalid".to_string(),
            producer: "context-tests".to_string(),
            causation_id: None,
            metadata: BTreeMap::new(),
        };

        assert!(envelope.validate().is_err());
    }

    #[test]
    fn payload_object_rejects_non_object_payload() {
        let envelope = EventEnvelope {
            event_type: "context.update.request".to_string(),
            payload: json!("not-an-object"),
            idempotency_key: "idemp-1".to_string(),
            correlation_id: "corr-1".to_string(),
            timestamp: "2026-03-09T19:30:00+00:00".to_string(),
            producer: "context-tests".to_string(),
            causation_id: None,
            metadata: BTreeMap::new(),
        };

        assert!(envelope.payload_object().is_err());
    }
}
