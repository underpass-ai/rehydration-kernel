#![cfg(feature = "container-tests")]

use std::error::Error;
use std::time::Duration;

use rehydration_adapter_nats::NatsCompatibilityRuntime;
use serde_json::{Value, json};
use testcontainers::core::IntoContainerPort;
use tokio::time::timeout;
use tokio_stream::StreamExt;

mod support;

use support::{
    NATS_INTERNAL_PORT, connect_with_retry, enveloped_payload, seeded_service, start_nats_container,
};

#[tokio::test]
async fn runtime_processes_update_requests_and_publishes_responses() {
    let (_container, url, runtime) = start_runtime().await.expect("runtime should start");
    let publisher = runtime.context_updated_publisher();
    let runtime_handle = tokio::spawn(runtime.run());
    let client = connect_with_retry(&url)
        .await
        .expect("client should connect");

    let mut response_subscription = client
        .subscribe("context.update.response".to_string())
        .await
        .expect("response subscription should succeed");
    let mut updated_subscription = client
        .subscribe("context.events.updated".to_string())
        .await
        .expect("updated subscription should succeed");

    client
        .publish(
            "context.update.request".to_string(),
            enveloped_payload(
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
            )
            .to_string()
            .into_bytes()
            .into(),
        )
        .await
        .expect("request publish should succeed");
    client.flush().await.expect("flush should succeed");

    let response = timeout(Duration::from_secs(10), response_subscription.next())
        .await
        .expect("response should arrive before timeout")
        .expect("response subscription should stay open");
    let response_envelope: Value =
        serde_json::from_slice(&response.payload).expect("response payload must be valid json");
    assert_eq!(response_envelope["event_type"], "context.update.response");
    assert_eq!(response_envelope["payload"]["story_id"], "story-1");
    assert_eq!(response_envelope["payload"]["status"], "success");

    publisher
        .publish("story-1", 7)
        .await
        .expect("context updated publish should succeed");

    let updated = timeout(Duration::from_secs(10), updated_subscription.next())
        .await
        .expect("updated event should arrive before timeout")
        .expect("updated subscription should stay open");
    let updated_envelope: Value =
        serde_json::from_slice(&updated.payload).expect("updated payload must be valid json");
    assert_eq!(updated_envelope["event_type"], "context.updated");
    assert_eq!(updated_envelope["payload"]["story_id"], "story-1");
    assert_eq!(updated_envelope["payload"]["version"], 7);

    runtime_handle.abort();
}

async fn start_runtime() -> Result<
    (
        testcontainers::ContainerAsync<testcontainers::GenericImage>,
        String,
        NatsCompatibilityRuntime<
            rehydration_adapter_nats::ContextAsyncApplication<
                rehydration_testkit::InMemoryGraphNeighborhoodReader,
                rehydration_testkit::InMemoryNodeDetailReader,
                rehydration_testkit::NoopSnapshotStore,
            >,
        >,
    ),
    Box<dyn Error + Send + Sync>,
> {
    let container = start_nats_container().await?;
    let port = container
        .get_host_port_ipv4(NATS_INTERNAL_PORT.tcp())
        .await?;
    let url = format!("nats://127.0.0.1:{port}");
    let runtime = NatsCompatibilityRuntime::connect(&url, seeded_service()).await?;

    Ok((container, url, runtime))
}
