#![cfg(feature = "container-tests")]
#![allow(dead_code)]

mod agentic_support;

use agentic_support::agentic_debug::debug_log;
use agentic_support::agentic_fixture::AgenticFixture;
use agentic_support::llm_planner::LlmPlanner;
use agentic_support::llm_starship_agent::{LlmStarshipMissionAgent, LlmStarshipMissionRequest};
use agentic_support::nats_container::connect_with_retry;
use agentic_support::runtime_workspace::RecordingRuntime;
use agentic_support::starship_seed_data::{
    CAPTAINS_LOG_PATH, MISSION_ROOT_NODE_ID, MISSION_ROOT_NODE_KIND, ROUTE_COMMAND_PATH,
    SCAN_COMMAND_PATH, STARSHIP_STATE_PATH, STARSHIP_TEST_PATH, STATUS_COMMAND_PATH,
    STEP_ONE_NODE_ID, STEP_TWO_NODE_ID, publish_initial_projection_events,
    publish_resume_projection_events,
};

#[tokio::test]
#[ignore = "requires a running external LLM endpoint"]
async fn real_llm_runs_starship_demo_resume_flow() {
    debug_log("starting real llm starship demo");
    let fixture = AgenticFixture::start_with_seed(
        MISSION_ROOT_NODE_ID,
        STEP_ONE_NODE_ID,
        |publisher| async move { publish_initial_projection_events(&publisher).await },
    )
    .await
    .expect("agentic fixture should start with starship mission");
    let runtime = RecordingRuntime::default();
    let llm = LlmPlanner::from_env().expect("LLM environment should be configured");
    let request =
        LlmStarshipMissionRequest::reference_defaults(MISSION_ROOT_NODE_ID, MISSION_ROOT_NODE_KIND);

    let mut phase_one_agent = LlmStarshipMissionAgent::new(
        fixture.query_client(),
        fixture.admin_client(),
        runtime.clone(),
        llm.clone(),
    );
    let phase_one = phase_one_agent
        .execute_next_step(request.clone())
        .await
        .expect("phase one should complete with a real LLM");

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
            .expect("state file should exist")
            .contains('{')
    );
    assert!(
        runtime
            .read_file(SCAN_COMMAND_PATH)
            .expect("scan file should be readable")
            .expect("scan file should exist")
            .to_lowercase()
            .contains("scan")
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

    let mut resumed_agent = LlmStarshipMissionAgent::new(
        fixture.query_client(),
        fixture.admin_client(),
        runtime.clone(),
        llm,
    );
    let phase_two = resumed_agent
        .execute_next_step(request)
        .await
        .expect("phase two should resume with a real LLM");

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
    assert!(captains_log.to_lowercase().contains("starship"));

    let phase_two_writes = runtime
        .invocations()
        .expect("phase two invocations should be readable")
        .into_iter()
        .filter(|invocation| invocation.tool_name == "fs.write")
        .map(|invocation| invocation.path.expect("write calls should carry a path"))
        .collect::<Vec<_>>();
    assert!(
        !phase_two_writes
            .iter()
            .any(|path| path == SCAN_COMMAND_PATH)
    );

    fixture.shutdown().await.expect("fixture should shut down");
    debug_log("finished real llm starship demo");
}
