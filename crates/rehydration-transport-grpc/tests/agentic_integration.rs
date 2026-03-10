#![cfg(feature = "container-tests")]

mod agentic_support;

use agentic_support::agentic_debug::debug_log;
use agentic_support::agentic_fixture::AgenticFixture;
use agentic_support::basic_context_agent::{AgentRequest, BasicContextAgent, SUMMARY_PATH};
use agentic_support::fake_underpass_runtime::FakeUnderpassRuntime;
use agentic_support::generic_seed_data::{
    FOCUS_DETAIL, FOCUS_NODE_ID, FOCUS_TITLE, ROOT_NODE_ID, ROOT_NODE_KIND, ROOT_TITLE,
};
use agentic_support::runtime_workspace::RecordingRuntime;
use agentic_support::underpass_runtime_client::UnderpassRuntimeClient;

#[tokio::test]
async fn basic_agent_uses_kernel_context_to_drive_runtime_actions() {
    debug_log("starting test basic_agent_uses_kernel_context_to_drive_runtime_actions");
    let fixture = AgenticFixture::start()
        .await
        .expect("agentic fixture should start");
    let runtime = RecordingRuntime::default();
    let mut agent = BasicContextAgent::new(fixture.query_client(), fixture.admin_client(), runtime);

    let execution = agent
        .execute(AgentRequest::reference_defaults(
            ROOT_NODE_ID,
            ROOT_NODE_KIND,
        ))
        .await
        .expect("agent should complete with kernel context");

    assert_eq!(execution.selected_node_id, FOCUS_NODE_ID);
    assert_eq!(execution.written_path, SUMMARY_PATH);
    assert!(execution.listed_files.contains(SUMMARY_PATH));
    assert!(execution.written_content.contains(ROOT_TITLE));
    assert!(execution.written_content.contains(FOCUS_TITLE));
    assert!(execution.written_content.contains(FOCUS_DETAIL));

    let recorded_file = agent
        .runtime()
        .read_file(SUMMARY_PATH)
        .expect("recording runtime should expose the written file")
        .expect("summary file should exist");
    assert!(recorded_file.contains("# Context Summary"));

    fixture.shutdown().await.expect("fixture should shut down");
    debug_log("finished test basic_agent_uses_kernel_context_to_drive_runtime_actions");
}

#[tokio::test]
async fn basic_agent_uses_underpass_runtime_contract_with_kernel_context() {
    debug_log("starting test basic_agent_uses_underpass_runtime_contract_with_kernel_context");
    let fixture = AgenticFixture::start()
        .await
        .expect("agentic fixture should start");
    let runtime_server = FakeUnderpassRuntime::start()
        .await
        .expect("runtime server should start");
    let runtime = UnderpassRuntimeClient::connect(runtime_server.base_url())
        .await
        .expect("runtime client should connect");
    let mut agent = BasicContextAgent::new(fixture.query_client(), fixture.admin_client(), runtime);

    let execution = agent
        .execute(AgentRequest::reference_defaults(
            ROOT_NODE_ID,
            ROOT_NODE_KIND,
        ))
        .await
        .expect("agent should complete with runtime http contract");

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
    debug_log("finished test basic_agent_uses_underpass_runtime_contract_with_kernel_context");
}
