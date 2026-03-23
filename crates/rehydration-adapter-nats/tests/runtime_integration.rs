#![cfg(feature = "container-tests")]

use std::time::Duration;

use rehydration_adapter_nats::{NatsClientTlsConfig, NatsProjectionRuntime, NatsRuntimeError};
use rehydration_application::{
    ApplicationError, ProjectionEventHandler, ProjectionHandlingRequest, ProjectionHandlingResult,
};
use serde_json::{Value, json};
use testcontainers::core::IntoContainerPort;
use tokio::sync::Mutex;
use tokio::time::{sleep, timeout};

mod support;

use support::{NATS_INTERNAL_PORT, connect_with_retry, start_nats_container};

#[derive(Debug, Default, Clone)]
struct RecordingProjectionHandler {
    requests: std::sync::Arc<Mutex<Vec<ProjectionHandlingRequest>>>,
    fail: bool,
}

impl RecordingProjectionHandler {
    async fn requests(&self) -> Vec<ProjectionHandlingRequest> {
        self.requests.lock().await.clone()
    }
}

impl ProjectionEventHandler for RecordingProjectionHandler {
    async fn handle_projection_event(
        &self,
        request: ProjectionHandlingRequest,
    ) -> Result<ProjectionHandlingResult, ApplicationError> {
        self.requests.lock().await.push(request.clone());
        if self.fail {
            return Err(ApplicationError::Validation(
                "projection failed intentionally".to_string(),
            ));
        }

        Ok(ProjectionHandlingResult {
            event_id: request.event.event_id().to_string(),
            subject: request.subject,
            duplicate: false,
            applied_mutations: 1,
            checkpoint: None,
        })
    }
}

#[tokio::test]
async fn projection_runtime_processes_graph_and_detail_events() {
    let container = start_nats_container()
        .await
        .expect("container should start");
    let port = container
        .get_host_port_ipv4(NATS_INTERNAL_PORT.tcp())
        .await
        .expect("nats port should resolve");
    let url = format!("nats://127.0.0.1:{port}");
    let handler = RecordingProjectionHandler::default();
    let runtime = connect_projection_runtime_with_retry(&url, "rehydration", handler.clone())
        .await
        .expect("projection runtime should connect");
    assert!(runtime.describe().contains("graph.node.materialized"));
    let runtime_handle = tokio::spawn(runtime.run());
    let client = connect_with_retry(&url)
        .await
        .expect("client should connect");

    client
        .publish(
            "rehydration.graph.node.materialized".to_string(),
            projection_graph_payload("evt-graph")
                .to_string()
                .into_bytes()
                .into(),
        )
        .await
        .expect("graph publish should succeed");
    client
        .publish(
            "rehydration.node.detail.materialized".to_string(),
            projection_detail_payload("evt-detail")
                .to_string()
                .into_bytes()
                .into(),
        )
        .await
        .expect("detail publish should succeed");
    client.flush().await.expect("flush should succeed");

    let requests = timeout(Duration::from_secs(10), async {
        loop {
            let requests = handler.requests().await;
            if requests.len() == 2 {
                break requests;
            }
            sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("projection runtime should consume both events");

    assert!(
        requests
            .iter()
            .all(|request| request.stream_name == "rehydration.events")
    );
    let subjects: std::collections::BTreeSet<_> = requests
        .iter()
        .map(|request| request.subject.as_str())
        .collect();
    assert_eq!(
        subjects,
        std::collections::BTreeSet::from(["graph.node.materialized", "node.detail.materialized",])
    );

    runtime_handle.abort();
}

#[tokio::test]
async fn projection_runtime_surfaces_connection_errors() {
    let error = timeout(
        Duration::from_secs(5),
        NatsProjectionRuntime::connect(
            "nats://127.0.0.1:1",
            &NatsClientTlsConfig::disabled(),
            "rehydration",
            RecordingProjectionHandler::default(),
        ),
    )
    .await
    .expect("connect should fail before timeout")
    .expect_err("connect should fail");

    assert!(matches!(error, NatsRuntimeError::Connection(_)));
}

#[tokio::test]
async fn projection_runtime_naks_failed_events_and_stops_with_consumer_error() {
    let container = start_nats_container()
        .await
        .expect("container should start");
    let port = container
        .get_host_port_ipv4(NATS_INTERNAL_PORT.tcp())
        .await
        .expect("nats port should resolve");
    let url = format!("nats://127.0.0.1:{port}");
    let runtime = connect_projection_runtime_with_retry(
        &url,
        "rehydration",
        RecordingProjectionHandler {
            requests: Default::default(),
            fail: true,
        },
    )
    .await
    .expect("projection runtime should connect");
    let runtime_handle = tokio::spawn(runtime.run());
    let client = connect_with_retry(&url)
        .await
        .expect("client should connect");

    client
        .publish(
            "rehydration.graph.node.materialized".to_string(),
            projection_graph_payload("evt-error")
                .to_string()
                .into_bytes()
                .into(),
        )
        .await
        .expect("graph publish should succeed");
    client.flush().await.expect("flush should succeed");

    let error = timeout(Duration::from_secs(10), runtime_handle)
        .await
        .expect("projection runtime should stop after consumer error")
        .expect("join should succeed")
        .expect_err("runtime should return an error");
    assert!(matches!(error, NatsRuntimeError::Consumer(_)));
}

async fn connect_projection_runtime_with_retry<H>(
    url: &str,
    subject_prefix: &str,
    handler: H,
) -> Result<NatsProjectionRuntime<H>, NatsRuntimeError>
where
    H: ProjectionEventHandler + Send + Sync + Clone + 'static,
{
    let mut last_error: Option<NatsRuntimeError> = None;

    for _ in 0..30 {
        match NatsProjectionRuntime::connect(
            url,
            &NatsClientTlsConfig::disabled(),
            subject_prefix,
            handler.clone(),
        )
        .await
        {
            Ok(runtime) => return Ok(runtime),
            Err(NatsRuntimeError::Connection(error)) => {
                last_error = Some(NatsRuntimeError::Connection(error));
                sleep(Duration::from_secs(1)).await;
            }
            Err(error) => return Err(error),
        }
    }

    Err(last_error.expect("at least one projection runtime connection attempt should fail"))
}

fn projection_graph_payload(event_id: &str) -> Value {
    json!({
        "event_id": event_id,
        "correlation_id": format!("corr-{event_id}"),
        "causation_id": format!("cmd-{event_id}"),
        "occurred_at": "2026-03-12T00:00:00Z",
        "aggregate_id": "node-root",
        "aggregate_type": "node",
        "schema_version": "v1beta1",
        "data": {
            "node_id": "node-root",
            "node_kind": "capability",
            "title": "Root capability",
            "summary": "projection runtime test",
            "status": "ACTIVE",
            "labels": ["projection"],
            "properties": {"phase": "build"},
            "related_nodes": [{
                "node_id": "node-detail",
                "relation_type": "depends_on",
                "explanation": {
                    "semantic_class": "constraint",
                    "sequence": 1
                }
            }]
        }
    })
}

fn projection_detail_payload(event_id: &str) -> Value {
    json!({
        "event_id": event_id,
        "correlation_id": format!("corr-{event_id}"),
        "causation_id": format!("cmd-{event_id}"),
        "occurred_at": "2026-03-12T00:00:01Z",
        "aggregate_id": "node-detail",
        "aggregate_type": "node",
        "schema_version": "v1beta1",
        "data": {
            "node_id": "node-detail",
            "detail": "Expanded node detail",
            "content_hash": "hash-123",
            "revision": 2
        }
    })
}
