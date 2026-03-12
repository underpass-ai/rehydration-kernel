#![allow(unused_imports)]

pub(crate) use rehydration_transport_grpc::starship_demo::{
    CAPTAINS_LOG_PATH, MISSION_ROOT_NODE_ID, MISSION_ROOT_NODE_KIND, MISSION_ROOT_TITLE,
    REPAIR_COMMAND_PATH, ROUTE_COMMAND_PATH, SCAN_COMMAND_PATH, STARSHIP_STATE_PATH,
    STARSHIP_TEST_PATH, STATUS_COMMAND_PATH, STEP_ONE_DETAIL, STEP_ONE_NODE_ID, STEP_ONE_TITLE,
    STEP_TWO_DETAIL, STEP_TWO_NODE_ID, STEP_TWO_TITLE, StarshipScenario,
};

pub(crate) async fn publish_initial_projection_events(
    client: &async_nats::Client,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    rehydration_transport_grpc::starship_demo::publish_initial_projection_events(client).await
}

pub(crate) async fn publish_resume_projection_events(
    client: &async_nats::Client,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    rehydration_transport_grpc::starship_demo::publish_resume_projection_events(client).await
}
