use std::error::Error;

use rehydration_proto::fleet_context_v1::GetGraphRelationshipsRequest;

use crate::support::golden_contract::expected_get_graph_relationships_response;
use crate::support::seed_data::{ROOT_LABEL, ROOT_NODE_ID};
use crate::support::seeded_fixture::SeededCompatibilityFixture;

#[tokio::test]
async fn grpc_get_graph_relationships_matches_golden_contract_after_depth_clamp()
-> Result<(), Box<dyn Error + Send + Sync>> {
    let mut fixture = SeededCompatibilityFixture::start().await?;

    let result = async {
        let response = fixture
            .client()
            .get_graph_relationships(GetGraphRelationshipsRequest {
                node_id: ROOT_NODE_ID.to_string(),
                node_type: ROOT_LABEL.to_string(),
                depth: 9,
            })
            .await?
            .into_inner();

        assert_eq!(response, expected_get_graph_relationships_response());
        Ok::<(), Box<dyn Error + Send + Sync>>(())
    }
    .await;

    fixture.shutdown().await?;
    result
}
