use std::error::Error;

use rehydration_proto::v1beta1::GetGraphRelationshipsRequest;

use crate::support::kernel_golden_contract::{
    expected_get_graph_relationships_response, normalize_get_graph_relationships_response,
};
use crate::support::seed_data::{ROOT_NODE_ID, ROOT_NODE_KIND};
use crate::support::seeded_kernel_fixture::SeededKernelFixture;

#[tokio::test]
async fn grpc_get_graph_relationships_matches_v1beta1_golden_contract()
-> Result<(), Box<dyn Error + Send + Sync>> {
    let mut fixture = SeededKernelFixture::start().await?;

    let result = async {
        let response = fixture
            .admin_client()
            .get_graph_relationships(GetGraphRelationshipsRequest {
                node_id: ROOT_NODE_ID.to_string(),
                node_kind: ROOT_NODE_KIND.to_string(),
                depth: 9,
                include_reverse_edges: false,
            })
            .await?
            .into_inner();

        assert_eq!(
            normalize_get_graph_relationships_response(response),
            expected_get_graph_relationships_response()
        );
        Ok::<(), Box<dyn Error + Send + Sync>>(())
    }
    .await;

    fixture.shutdown().await?;
    result
}
