#![cfg(feature = "container-tests")]

mod agentic_event_support;
mod agentic_support {
    pub(crate) use crate::agentic_event_support::*;
}

use std::time::Duration;

use agentic_event_support::agentic_debug::debug_log;
use agentic_event_support::agentic_fixture::AgenticFixture;
use agentic_event_support::context_bundle_generated_event::publish_context_bundle_generated_event;
use agentic_event_support::event_driven_runtime_trigger::EventDrivenRuntimeTrigger;
use agentic_event_support::fake_underpass_runtime::FakeUnderpassRuntime;
use agentic_event_support::generic_seed_data::{
    FOCUS_DETAIL, FOCUS_NODE_ID, FOCUS_TITLE, ROOT_NODE_ID, ROOT_NODE_KIND, ROOT_TITLE,
};
use agentic_event_support::nats_container::connect_with_retry;
use agentic_event_support::runtime_workspace::RecordingRuntime;
use rehydration_transport_grpc::agentic_reference::{
    AgentRequest, SUMMARY_PATH, UnderpassRuntimeClient,
};

#[tokio::test]
async fn bundle_generated_event_triggers_recording_runtime_workflow() {
    debug_log("starting test bundle_generated_event_triggers_recording_runtime_workflow");
    let fixture = AgenticFixture::start()
        .await
        .expect("agentic fixture should start");
    let runtime = RecordingRuntime::default();
    let runtime_observer = runtime.clone();
    let trigger = EventDrivenRuntimeTrigger::new(
        fixture.query_client(),
        fixture.admin_client(),
        runtime,
        fixture.nats_url(),
        AgentRequest::reference_defaults(ROOT_NODE_ID, ROOT_NODE_KIND),
    );
    let (trigger_task, ready_rx) = trigger.spawn();
    ready_rx
        .await
        .expect("bundle generated subscription should become ready");

    let publisher = connect_with_retry(fixture.nats_url())
        .await
        .expect("nats publisher should connect");
    publish_context_bundle_generated_event(&publisher, &["implementer"])
        .await
        .expect("bundle generated event should publish");

    let execution = tokio::time::timeout(Duration::from_secs(20), trigger_task)
        .await
        .expect("event-driven trigger should complete before timeout")
        .expect("trigger task should join")
        .expect("trigger should complete successfully");

    assert_eq!(execution.selected_node_id, FOCUS_NODE_ID);
    assert_eq!(execution.written_path, SUMMARY_PATH);
    assert!(execution.listed_files.contains(SUMMARY_PATH));
    assert!(execution.written_content.contains(ROOT_TITLE));
    assert!(execution.written_content.contains(FOCUS_TITLE));
    assert!(execution.written_content.contains(FOCUS_DETAIL));
    let recorded_file = runtime_observer
        .read_file(SUMMARY_PATH)
        .expect("recording runtime should expose the written file")
        .expect("summary file should exist");
    assert!(recorded_file.contains("# Context Summary"));

    fixture.shutdown().await.expect("fixture should shut down");
    debug_log("finished test bundle_generated_event_triggers_recording_runtime_workflow");
}

#[tokio::test]
async fn bundle_generated_event_triggers_underpass_runtime_contract_workflow() {
    debug_log("starting test bundle_generated_event_triggers_underpass_runtime_contract_workflow");
    let fixture = AgenticFixture::start()
        .await
        .expect("agentic fixture should start");
    let runtime_server = FakeUnderpassRuntime::start()
        .await
        .expect("runtime server should start");
    let runtime = UnderpassRuntimeClient::connect(runtime_server.base_url())
        .await
        .expect("runtime client should connect");
    let trigger = EventDrivenRuntimeTrigger::new(
        fixture.query_client(),
        fixture.admin_client(),
        runtime,
        fixture.nats_url(),
        AgentRequest::reference_defaults(ROOT_NODE_ID, ROOT_NODE_KIND),
    );
    let (trigger_task, ready_rx) = trigger.spawn();
    ready_rx
        .await
        .expect("bundle generated subscription should become ready");

    let publisher = connect_with_retry(fixture.nats_url())
        .await
        .expect("nats publisher should connect");
    publish_context_bundle_generated_event(&publisher, &["implementer"])
        .await
        .expect("bundle generated event should publish");

    let execution = tokio::time::timeout(Duration::from_secs(20), trigger_task)
        .await
        .expect("event-driven trigger should complete before timeout")
        .expect("trigger task should join")
        .expect("trigger should complete successfully");

    assert_eq!(execution.selected_node_id, FOCUS_NODE_ID);
    assert_eq!(execution.written_path, SUMMARY_PATH);
    assert!(execution.listed_files.contains(SUMMARY_PATH));
    assert!(execution.written_content.contains(ROOT_TITLE));
    assert!(execution.written_content.contains(FOCUS_TITLE));
    assert!(execution.written_content.contains(FOCUS_DETAIL));

    let runtime_file = runtime_server
        .read_file(SUMMARY_PATH)
        .expect("runtime file should be readable");
    assert!(runtime_file.is_some());

    runtime_server
        .shutdown()
        .await
        .expect("runtime server should shut down");
    fixture.shutdown().await.expect("fixture should shut down");
    debug_log("finished test bundle_generated_event_triggers_underpass_runtime_contract_workflow");
}
