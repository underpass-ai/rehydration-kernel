use std::sync::Arc;

use rehydration_adapter_nats::{
    ContextAsyncService, NatsConsumerError, NatsContextCompatibilityConsumer, NatsPublication,
    NatsPublicationSink, NatsRequestMessage,
};
use serde_json::{Value, json};
use tokio::sync::Mutex;

mod support;

use support::{enveloped_payload, seeded_service};

#[derive(Debug)]
struct TestMessage {
    payload: Vec<u8>,
    acked: Arc<Mutex<u32>>,
    naked: Arc<Mutex<u32>>,
}

impl TestMessage {
    fn new(payload: Value) -> Self {
        Self {
            payload: payload.to_string().into_bytes(),
            acked: Arc::new(Mutex::new(0)),
            naked: Arc::new(Mutex::new(0)),
        }
    }

    fn from_bytes(payload: &[u8]) -> Self {
        Self {
            payload: payload.to_vec(),
            acked: Arc::new(Mutex::new(0)),
            naked: Arc::new(Mutex::new(0)),
        }
    }

    async fn ack_count(&self) -> u32 {
        *self.acked.lock().await
    }

    async fn nak_count(&self) -> u32 {
        *self.naked.lock().await
    }
}

impl NatsRequestMessage for TestMessage {
    fn payload(&self) -> &[u8] {
        &self.payload
    }

    async fn ack(&self) -> Result<(), NatsConsumerError> {
        *self.acked.lock().await += 1;
        Ok(())
    }

    async fn nak(&self) -> Result<(), NatsConsumerError> {
        *self.naked.lock().await += 1;
        Ok(())
    }
}

#[derive(Debug, Default)]
struct RecordingPublisher {
    publications: Mutex<Vec<NatsPublication>>,
}

impl RecordingPublisher {
    async fn publications(&self) -> Vec<NatsPublication> {
        self.publications.lock().await.clone()
    }
}

impl NatsPublicationSink for RecordingPublisher {
    async fn publish(&self, publication: NatsPublication) -> Result<(), NatsConsumerError> {
        self.publications.lock().await.push(publication);
        Ok(())
    }
}

#[derive(Debug)]
struct FailingService;

impl ContextAsyncService for FailingService {
    async fn update_context(
        &self,
        _command: rehydration_application::UpdateContextCommand,
    ) -> Result<
        rehydration_application::UpdateContextOutcome,
        rehydration_application::ApplicationError,
    > {
        Err(rehydration_application::ApplicationError::Validation(
            "update failed".to_string(),
        ))
    }

    async fn rehydrate_session(
        &self,
        _query: rehydration_application::RehydrateSessionQuery,
    ) -> Result<
        rehydration_application::RehydrateSessionResult,
        rehydration_application::ApplicationError,
    > {
        Err(rehydration_application::ApplicationError::Validation(
            "rehydrate failed".to_string(),
        ))
    }
}

#[test]
fn describe_mentions_external_async_subjects() {
    let consumer =
        NatsContextCompatibilityConsumer::new(FailingService, RecordingPublisher::default());

    assert!(consumer.describe().contains("context.update.request"));
    assert!(consumer.describe().contains("context.rehydrate.request"));
}

#[tokio::test]
async fn consume_update_request_publishes_response_and_acks() {
    let publisher = Arc::new(RecordingPublisher::default());
    let consumer =
        NatsContextCompatibilityConsumer::new(Arc::new(seeded_service()), Arc::clone(&publisher));
    let message = TestMessage::new(enveloped_payload(
        "context.update.request",
        json!({
            "story_id": "story-1",
            "task_id": "task-1",
            "role": "developer",
            "changes": [
                {
                    "operation": "UPDATE",
                    "entity_type": "decision",
                    "entity_id": "decision-1",
                    "payload": {"status": "accepted"},
                    "reason": "refined"
                }
            ]
        }),
    ));

    consumer
        .consume("context.update.request", &message)
        .await
        .expect("request should succeed");

    assert_eq!(message.ack_count().await, 1);
    assert_eq!(message.nak_count().await, 0);

    let publications = publisher.publications().await;
    assert_eq!(publications.len(), 1);
    assert_eq!(publications[0].subject, "context.update.response");

    let envelope: Value =
        serde_json::from_slice(&publications[0].payload).expect("publication must be json");
    assert_eq!(envelope["event_type"], "context.update.response");
    assert_eq!(envelope["payload"]["story_id"], "story-1");
    assert_eq!(envelope["payload"]["status"], "success");
}

#[tokio::test]
async fn consume_rehydrate_request_publishes_summary_response_and_acks() {
    let publisher = Arc::new(RecordingPublisher::default());
    let consumer =
        NatsContextCompatibilityConsumer::new(Arc::new(seeded_service()), Arc::clone(&publisher));
    let message = TestMessage::new(enveloped_payload(
        "context.rehydrate.request",
        json!({
            "case_id": "case-123",
            "roles": ["developer"],
            "timeline_events": 0,
            "persist_bundle": false,
            "ttl_seconds": 0
        }),
    ));

    consumer
        .consume("context.rehydrate.request", &message)
        .await
        .expect("request should succeed");

    assert_eq!(message.ack_count().await, 1);
    assert_eq!(message.nak_count().await, 0);

    let publications = publisher.publications().await;
    assert_eq!(publications.len(), 1);
    assert_eq!(publications[0].subject, "context.rehydrate.response");

    let envelope: Value =
        serde_json::from_slice(&publications[0].payload).expect("publication must be json");
    assert_eq!(envelope["event_type"], "context.rehydrate.response");
    assert_eq!(envelope["payload"]["case_id"], "case-123");
    assert_eq!(envelope["payload"]["status"], "success");
    assert_eq!(envelope["payload"]["packs_count"], 1);
    assert_eq!(envelope["payload"]["stats"]["events"], 50);
}

#[tokio::test]
async fn consume_update_request_acks_invalid_json_and_drops() {
    let publisher = Arc::new(RecordingPublisher::default());
    let consumer =
        NatsContextCompatibilityConsumer::new(Arc::new(seeded_service()), Arc::clone(&publisher));
    let message = TestMessage::from_bytes(b"not-json");

    consumer
        .consume("context.update.request", &message)
        .await
        .expect("invalid json should be dropped");

    assert_eq!(message.ack_count().await, 1);
    assert_eq!(message.nak_count().await, 0);
    assert!(publisher.publications().await.is_empty());
}

#[tokio::test]
async fn consume_rehydrate_request_acks_invalid_envelope_and_drops() {
    let publisher = Arc::new(RecordingPublisher::default());
    let consumer =
        NatsContextCompatibilityConsumer::new(Arc::new(seeded_service()), Arc::clone(&publisher));
    let message = TestMessage::new(json!({"case_id": "case-123"}));

    consumer
        .consume("context.rehydrate.request", &message)
        .await
        .expect("invalid envelope should be dropped");

    assert_eq!(message.ack_count().await, 1);
    assert_eq!(message.nak_count().await, 0);
    assert!(publisher.publications().await.is_empty());
}

#[tokio::test]
async fn consume_update_request_acks_non_object_payload_and_drops() {
    let publisher = Arc::new(RecordingPublisher::default());
    let consumer =
        NatsContextCompatibilityConsumer::new(Arc::new(seeded_service()), Arc::clone(&publisher));
    let message = TestMessage::new(enveloped_payload(
        "context.update.request",
        json!(["not", "an", "object"]),
    ));

    consumer
        .consume("context.update.request", &message)
        .await
        .expect("non-object payload should be dropped");

    assert_eq!(message.ack_count().await, 1);
    assert_eq!(message.nak_count().await, 0);
    assert!(publisher.publications().await.is_empty());
}

#[tokio::test]
async fn consume_update_request_naks_on_service_error() {
    let publisher = Arc::new(RecordingPublisher::default());
    let consumer =
        NatsContextCompatibilityConsumer::new(Arc::new(FailingService), Arc::clone(&publisher));
    let message = TestMessage::new(enveloped_payload(
        "context.update.request",
        json!({"story_id": "story-1"}),
    ));

    let error = consumer
        .consume("context.update.request", &message)
        .await
        .expect_err("service error should nak");

    assert!(matches!(error, NatsConsumerError::Application(_)));
    assert_eq!(message.ack_count().await, 0);
    assert_eq!(message.nak_count().await, 1);
}

#[tokio::test]
async fn consume_rehydrate_request_naks_on_service_error() {
    let publisher = Arc::new(RecordingPublisher::default());
    let consumer =
        NatsContextCompatibilityConsumer::new(Arc::new(FailingService), Arc::clone(&publisher));
    let message = TestMessage::new(enveloped_payload(
        "context.rehydrate.request",
        json!({"case_id": "case-123"}),
    ));

    let error = consumer
        .consume("context.rehydrate.request", &message)
        .await
        .expect_err("service error should nak");

    assert!(matches!(error, NatsConsumerError::Application(_)));
    assert_eq!(message.ack_count().await, 0);
    assert_eq!(message.nak_count().await, 1);
}
