use std::error::Error;

use rehydration_proto::fleet_context_v1::GetContextRequest;

use crate::support::golden_contract::expected_get_context_response;
use crate::support::seed_data::{BUILD_PHASE, DEVELOPER_ROLE, ROOT_NODE_ID};
use crate::support::seeded_fixture::SeededCompatibilityFixture;

#[tokio::test]
async fn grpc_get_context_basic_matches_golden_contract() -> Result<(), Box<dyn Error + Send + Sync>>
{
    let mut fixture = SeededCompatibilityFixture::start().await?;

    let result = async {
        let response = fixture
            .client()
            .get_context(GetContextRequest {
                story_id: ROOT_NODE_ID.to_string(),
                role: DEVELOPER_ROLE.to_string(),
                phase: BUILD_PHASE.to_string(),
                subtask_id: String::new(),
                token_budget: 2048,
            })
            .await?
            .into_inner();

        assert_eq!(response, expected_get_context_response());
        Ok::<(), Box<dyn Error + Send + Sync>>(())
    }
    .await;

    fixture.shutdown().await?;
    result
}
