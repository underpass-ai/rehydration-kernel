#![cfg(feature = "container-tests")]

use std::error::Error;
use std::time::Duration;

use rehydration_adapter_nats::{ContextAsyncService, NatsCompatibilityRuntime, NatsRuntimeError};
use rehydration_application::{
    ApplicationError, RehydrateSessionQuery, RehydrateSessionResult, UpdateContextCommand,
    UpdateContextOutcome,
};
use serde_json::{Value, json};
use testcontainers::core::IntoContainerPort;
use tokio::time::{sleep, timeout};
use tokio_stream::StreamExt;

mod support;

use support::{
    NATS_INTERNAL_PORT, connect_with_retry, enveloped_payload, seeded_service, start_nats_container,
};

struct ErroringService;

impl ContextAsyncService for ErroringService {
    async fn update_context(
        &self,
        _command: UpdateContextCommand,
    ) -> Result<UpdateContextOutcome, ApplicationError> {
        Err(ApplicationError::Validation(
            "update failed intentionally".to_string(),
        ))
    }

    async fn rehydrate_session(
        &self,
        _query: RehydrateSessionQuery,
    ) -> Result<RehydrateSessionResult, ApplicationError> {
        Err(ApplicationError::Validation(
            "rehydrate failed intentionally".to_string(),
        ))
    }
}

#[tokio::test]
async fn runtime_processes_update_requests_and_publishes_responses() {
    let (_container, url, runtime) = start_runtime().await.expect("runtime should start");
    assert!(runtime.describe().contains("context.update.request"));
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

#[tokio::test]
async fn runtime_processes_rehydrate_requests_and_publishes_responses() {
    let (_container, url, runtime) = start_runtime().await.expect("runtime should start");
    let runtime_handle = tokio::spawn(runtime.run());
    let client = connect_with_retry(&url)
        .await
        .expect("client should connect");

    let mut response_subscription = client
        .subscribe("context.rehydrate.response".to_string())
        .await
        .expect("response subscription should succeed");

    client
        .publish(
            "context.rehydrate.request".to_string(),
            enveloped_payload(
                "context.rehydrate.request",
                json!({
                    "case_id": "case-123",
                    "roles": ["developer"],
                    "include_timeline": true,
                    "include_summaries": true,
                    "timeline_events": 25,
                    "persist_bundle": false,
                    "ttl_seconds": 600
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
    assert_eq!(
        response_envelope["event_type"],
        "context.rehydrate.response"
    );
    assert_eq!(response_envelope["payload"]["case_id"], "case-123");
    assert_eq!(response_envelope["payload"]["packs_count"], 1);
    assert_eq!(response_envelope["payload"]["stats"]["events"], 25);

    runtime_handle.abort();
}

#[tokio::test]
async fn runtime_surfaces_connection_errors() {
    let error = timeout(
        Duration::from_secs(5),
        NatsCompatibilityRuntime::connect("nats://127.0.0.1:1", seeded_service()),
    )
    .await
    .expect("connect should fail before timeout")
    .expect_err("connect should fail");

    assert!(matches!(error, NatsRuntimeError::Connection(_)));
    assert!(error.to_string().contains("nats connection error"));
}

#[tokio::test]
async fn runtime_naks_failed_requests_and_stops_with_consumer_error() {
    let container = start_nats_container()
        .await
        .expect("container should start");
    let port = container
        .get_host_port_ipv4(NATS_INTERNAL_PORT.tcp())
        .await
        .expect("nats port should resolve");
    let url = format!("nats://127.0.0.1:{port}");
    let runtime = connect_runtime_with_retry(&url, || ErroringService)
        .await
        .expect("runtime should connect");
    let runtime_handle = tokio::spawn(runtime.run());
    let client = connect_with_retry(&url)
        .await
        .expect("client should connect");

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

    let error = timeout(Duration::from_secs(10), runtime_handle)
        .await
        .expect("runtime should stop after consumer error")
        .expect("join should succeed")
        .expect_err("runtime should return an error");
    assert!(matches!(error, NatsRuntimeError::Consumer(_)));
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
    let runtime = connect_runtime_with_retry(&url, seeded_service).await?;

    Ok((container, url, runtime))
}

async fn connect_runtime_with_retry<S, F>(
    url: &str,
    factory: F,
) -> Result<NatsCompatibilityRuntime<S>, NatsRuntimeError>
where
    S: ContextAsyncService + Send + Sync + 'static,
    F: Fn() -> S,
{
    let mut last_error: Option<NatsRuntimeError> = None;

    for _ in 0..30 {
        match NatsCompatibilityRuntime::connect(url, factory()).await {
            Ok(runtime) => return Ok(runtime),
            Err(NatsRuntimeError::Connection(error)) => {
                last_error = Some(NatsRuntimeError::Connection(error));
                sleep(Duration::from_secs(1)).await;
            }
            Err(error) => return Err(error),
        }
    }

    Err(last_error.expect("at least one runtime connection attempt should fail"))
}
