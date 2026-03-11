#![cfg(feature = "container-tests")]

mod agentic_support;

use std::time::Duration;

use agentic_support::agentic_debug::debug_log;
use agentic_support::agentic_fixture::AgenticFixture;
use agentic_support::fake_underpass_runtime::FakeUnderpassRuntime;
use agentic_support::nats_container::connect_with_retry;
use agentic_support::runtime_workspace::RecordingRuntime;
use agentic_support::starship_mission_agent::{StarshipMissionAgent, StarshipMissionRequest};
use agentic_support::starship_seed_data::{
    CAPTAINS_LOG_PATH, MISSION_ROOT_NODE_ID, MISSION_ROOT_NODE_KIND, MISSION_ROOT_TITLE,
    ROUTE_COMMAND_PATH, SCAN_COMMAND_PATH, STARSHIP_STATE_PATH, STARSHIP_TEST_PATH,
    STATUS_COMMAND_PATH, STEP_ONE_NODE_ID, STEP_TWO_DETAIL, STEP_TWO_NODE_ID, STEP_TWO_TITLE,
    publish_initial_projection_events, publish_resume_projection_events,
};
use rehydration_transport_grpc::agentic_reference::UnderpassRuntimeClient;
use tokio::time::sleep;

#[tokio::test]
async fn agent_rehydrates_starship_mission_and_continues_with_recording_runtime() {
    debug_log("starting starship recording runtime test");
    let fixture = AgenticFixture::start_with_seed(
        MISSION_ROOT_NODE_ID,
        STEP_ONE_NODE_ID,
        |publisher| async move { publish_initial_projection_events(&publisher).await },
    )
    .await
    .expect("agentic fixture should start with starship mission");
    let runtime = RecordingRuntime::default();
    let mut phase_one_agent = StarshipMissionAgent::new(
        fixture.query_client(),
        fixture.admin_client(),
        runtime.clone(),
    );
    let request =
        StarshipMissionRequest::reference_defaults(MISSION_ROOT_NODE_ID, MISSION_ROOT_NODE_KIND);

    let phase_one = phase_one_agent
        .execute_next_step(request.clone())
        .await
        .expect("phase one should complete");

    assert_eq!(phase_one.selected_step_node_id, STEP_ONE_NODE_ID);
    assert_eq!(
        phase_one.written_paths,
        vec![
            SCAN_COMMAND_PATH.to_string(),
            "src/commands/repair.rs".to_string(),
            STARSHIP_STATE_PATH.to_string(),
        ]
    );
    assert!(
        runtime
            .read_file(STARSHIP_STATE_PATH)
            .expect("runtime state should be readable")
            .is_some()
    );
    assert!(
        runtime
            .read_file(SCAN_COMMAND_PATH)
            .expect("runtime file should be readable")
            .expect("scan command should exist")
            .contains("sensors online")
    );

    runtime
        .clear_invocations()
        .expect("phase one invocations should clear");

    let publisher = connect_with_retry(fixture.nats_url())
        .await
        .expect("nats publisher should connect");
    publish_resume_projection_events(&publisher)
        .await
        .expect("resume events should publish");

    let mut resumed_agent = StarshipMissionAgent::new(
        fixture.query_client(),
        fixture.admin_client(),
        runtime.clone(),
    );
    wait_for_current_step(&mut resumed_agent, &request, STEP_TWO_NODE_ID)
        .await
        .expect("step two should become current after rehydration");

    let phase_two = resumed_agent
        .execute_next_step(request)
        .await
        .expect("phase two should resume from rehydrated context");

    assert_eq!(phase_two.selected_step_node_id, STEP_TWO_NODE_ID);
    assert_eq!(
        phase_two.written_paths,
        vec![
            ROUTE_COMMAND_PATH.to_string(),
            STATUS_COMMAND_PATH.to_string(),
            STARSHIP_TEST_PATH.to_string(),
            CAPTAINS_LOG_PATH.to_string(),
        ]
    );
    let captains_log = phase_two
        .captains_log
        .expect("phase two should write captains-log.md");
    assert!(captains_log.contains(MISSION_ROOT_TITLE));
    assert!(captains_log.contains(STEP_TWO_TITLE));
    assert!(captains_log.contains(STEP_TWO_DETAIL));
    assert!(captains_log.contains("\"hull\":\"stabilized\""));

    let phase_two_writes = runtime
        .invocations()
        .expect("phase two invocations should be readable")
        .into_iter()
        .filter(|invocation| invocation.tool_name == "fs.write")
        .map(|invocation| invocation.path.expect("write calls should carry a path"))
        .collect::<Vec<_>>();
    assert_eq!(
        phase_two_writes,
        vec![
            ROUTE_COMMAND_PATH.to_string(),
            STATUS_COMMAND_PATH.to_string(),
            STARSHIP_TEST_PATH.to_string(),
            CAPTAINS_LOG_PATH.to_string(),
        ]
    );
    assert!(
        !phase_two_writes
            .iter()
            .any(|path| path == SCAN_COMMAND_PATH)
    );

    fixture.shutdown().await.expect("fixture should shut down");
    debug_log("finished starship recording runtime test");
}

#[tokio::test]
async fn agent_rehydrates_starship_mission_and_continues_with_runtime_http_contract() {
    debug_log("starting starship underpass runtime test");
    let fixture = AgenticFixture::start_with_seed(
        MISSION_ROOT_NODE_ID,
        STEP_ONE_NODE_ID,
        |publisher| async move { publish_initial_projection_events(&publisher).await },
    )
    .await
    .expect("agentic fixture should start with starship mission");
    let runtime_server = FakeUnderpassRuntime::start()
        .await
        .expect("runtime server should start");
    let runtime = UnderpassRuntimeClient::connect(runtime_server.base_url())
        .await
        .expect("runtime client should connect");
    let request =
        StarshipMissionRequest::reference_defaults(MISSION_ROOT_NODE_ID, MISSION_ROOT_NODE_KIND);

    let mut phase_one_agent =
        StarshipMissionAgent::new(fixture.query_client(), fixture.admin_client(), runtime);
    let phase_one = phase_one_agent
        .execute_next_step(request.clone())
        .await
        .expect("phase one should complete through the runtime contract");
    assert_eq!(phase_one.selected_step_node_id, STEP_ONE_NODE_ID);
    assert!(
        runtime_server
            .read_file(STARSHIP_STATE_PATH)
            .expect("runtime file should be readable")
            .is_some()
    );
    assert!(
        runtime_server
            .read_file(SCAN_COMMAND_PATH)
            .expect("runtime file should be readable")
            .expect("scan command should exist")
            .contains("sensors online")
    );

    let publisher = connect_with_retry(fixture.nats_url())
        .await
        .expect("nats publisher should connect");
    publish_resume_projection_events(&publisher)
        .await
        .expect("resume events should publish");

    let runtime = UnderpassRuntimeClient::connect(runtime_server.base_url())
        .await
        .expect("runtime client should reconnect");
    let mut resumed_agent =
        StarshipMissionAgent::new(fixture.query_client(), fixture.admin_client(), runtime);
    wait_for_current_step(&mut resumed_agent, &request, STEP_TWO_NODE_ID)
        .await
        .expect("step two should become current after rehydration");

    let phase_two = resumed_agent
        .execute_next_step(request)
        .await
        .expect("phase two should resume through the runtime contract");
    assert_eq!(phase_two.selected_step_node_id, STEP_TWO_NODE_ID);
    assert_eq!(
        phase_two.written_paths,
        vec![
            ROUTE_COMMAND_PATH.to_string(),
            STATUS_COMMAND_PATH.to_string(),
            STARSHIP_TEST_PATH.to_string(),
            CAPTAINS_LOG_PATH.to_string(),
        ]
    );

    let captains_log = runtime_server
        .read_file(CAPTAINS_LOG_PATH)
        .expect("runtime file should be readable")
        .expect("captains-log.md should exist");
    assert!(captains_log.contains(MISSION_ROOT_TITLE));
    assert!(captains_log.contains(STEP_TWO_TITLE));
    assert!(captains_log.contains("\"hull\":\"stabilized\""));

    runtime_server
        .shutdown()
        .await
        .expect("runtime server should shut down");
    fixture.shutdown().await.expect("fixture should shut down");
    debug_log("finished starship underpass runtime test");
}

async fn wait_for_current_step<R>(
    agent: &mut StarshipMissionAgent<R>,
    request: &StarshipMissionRequest,
    expected_step_node_id: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    R: agentic_support::runtime_workspace::AgentRuntime,
{
    for _ in 0..40 {
        let step = agent.current_step(request).await?;
        if step.node_id == expected_step_node_id {
            return Ok(());
        }
        sleep(Duration::from_millis(200)).await;
    }

    Err(
        format!("expected current step `{expected_step_node_id}` after starship rehydration")
            .into(),
    )
}
