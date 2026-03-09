use serde_json::{Value, json};

pub(crate) fn enveloped_payload(event_type: &str, payload: Value) -> Value {
    json!({
        "event_type": event_type,
        "payload": payload,
        "idempotency_key": "idemp-123",
        "correlation_id": "corr-456",
        "timestamp": "2026-03-09T19:30:00+00:00",
        "producer": "context-tests",
        "metadata": {}
    })
}
