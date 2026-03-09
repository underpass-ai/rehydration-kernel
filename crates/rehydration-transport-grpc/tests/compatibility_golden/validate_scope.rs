use std::error::Error;

use rehydration_proto::fleet_context_v1::{
    ValidateScopeRequest, context_service_client::ContextServiceClient,
};

use crate::support::empty_ports::{
    EmptyGraphNeighborhoodReader, EmptyNodeDetailReader, NoopSnapshotStore,
};
use crate::support::golden_contract::{
    expected_validate_scope_allowed_response, expected_validate_scope_rejected_response,
};
use crate::support::grpc_runtime::{RunningGrpcServer, stop_server};
use crate::support::seed_data::{
    BUILD_PHASE, DEVELOPER_ROLE, allowed_validate_scope_request_scopes,
    rejected_validate_scope_request_scopes,
};

#[tokio::test]
async fn grpc_validate_scope_allowed_matches_golden_contract()
-> Result<(), Box<dyn Error + Send + Sync>> {
    let server = RunningGrpcServer::start(
        EmptyGraphNeighborhoodReader,
        EmptyNodeDetailReader,
        NoopSnapshotStore,
    )
    .await?;
    let channel = server.connect_channel().await?;
    let mut client = ContextServiceClient::new(channel);

    let result = async {
        let response = client
            .validate_scope(ValidateScopeRequest {
                role: DEVELOPER_ROLE.to_string(),
                phase: BUILD_PHASE.to_string(),
                provided_scopes: allowed_validate_scope_request_scopes(),
            })
            .await?
            .into_inner();

        assert_eq!(response, expected_validate_scope_allowed_response());
        Ok::<(), Box<dyn Error + Send + Sync>>(())
    }
    .await;

    stop_server(server).await?;
    result
}

#[tokio::test]
async fn grpc_validate_scope_rejected_matches_golden_contract()
-> Result<(), Box<dyn Error + Send + Sync>> {
    let server = RunningGrpcServer::start(
        EmptyGraphNeighborhoodReader,
        EmptyNodeDetailReader,
        NoopSnapshotStore,
    )
    .await?;
    let channel = server.connect_channel().await?;
    let mut client = ContextServiceClient::new(channel);

    let result = async {
        let response = client
            .validate_scope(ValidateScopeRequest {
                role: DEVELOPER_ROLE.to_string(),
                phase: BUILD_PHASE.to_string(),
                provided_scopes: rejected_validate_scope_request_scopes(),
            })
            .await?
            .into_inner();

        assert_eq!(response, expected_validate_scope_rejected_response());
        Ok::<(), Box<dyn Error + Send + Sync>>(())
    }
    .await;

    stop_server(server).await?;
    result
}
