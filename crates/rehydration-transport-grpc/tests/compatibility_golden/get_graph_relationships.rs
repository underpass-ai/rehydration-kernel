use std::error::Error;

use rehydration_proto::fleet_context_v1::GetGraphRelationshipsRequest;
use tonic::Code;

use crate::support::empty_ports::{
    EmptyGraphNeighborhoodReader, EmptyNodeDetailReader, NoopSnapshotStore,
};
use crate::support::golden_contract::expected_get_graph_relationships_response;
use crate::support::grpc_runtime::{RunningGrpcServer, stop_server};
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

#[tokio::test]
async fn grpc_get_graph_relationships_rejects_invalid_node_type_with_invalid_argument()
-> Result<(), Box<dyn Error + Send + Sync>> {
    let server = RunningGrpcServer::start(
        EmptyGraphNeighborhoodReader,
        EmptyNodeDetailReader,
        NoopSnapshotStore,
    )
    .await?;
    let channel = server.connect_channel().await?;
    let mut client =
        rehydration_proto::fleet_context_v1::context_service_client::ContextServiceClient::new(
            channel,
        );

    let result = async {
        let error = client
            .get_graph_relationships(GetGraphRelationshipsRequest {
                node_id: ROOT_NODE_ID.to_string(),
                node_type: "InvalidType".to_string(),
                depth: 2,
            })
            .await
            .expect_err("invalid node type should be rejected");

        assert_eq!(error.code(), Code::InvalidArgument);
        assert_eq!(
            error.message(),
            "Invalid node_type: InvalidType. Must be Project, Epic, Story, or Task"
        );
        Ok::<(), Box<dyn Error + Send + Sync>>(())
    }
    .await;

    stop_server(server).await?;
    result
}

#[tokio::test]
async fn grpc_get_graph_relationships_rejects_missing_node_with_invalid_argument()
-> Result<(), Box<dyn Error + Send + Sync>> {
    let server = RunningGrpcServer::start(
        EmptyGraphNeighborhoodReader,
        EmptyNodeDetailReader,
        NoopSnapshotStore,
    )
    .await?;
    let channel = server.connect_channel().await?;
    let mut client =
        rehydration_proto::fleet_context_v1::context_service_client::ContextServiceClient::new(
            channel,
        );

    let result = async {
        let error = client
            .get_graph_relationships(GetGraphRelationshipsRequest {
                node_id: "missing-node".to_string(),
                node_type: ROOT_LABEL.to_string(),
                depth: 2,
            })
            .await
            .expect_err("missing node should be rejected");

        assert_eq!(error.code(), Code::InvalidArgument);
        assert_eq!(error.message(), "Node not found: missing-node");
        Ok::<(), Box<dyn Error + Send + Sync>>(())
    }
    .await;

    stop_server(server).await?;
    result
}
