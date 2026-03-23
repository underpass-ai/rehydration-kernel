use std::sync::Arc;

use rehydration_application::{
    AdminCommandApplicationService, AdminQueryApplicationService, GetBundleSnapshotQuery,
    GetGraphRelationshipsQuery, GetProjectionStatusQuery, GetRehydrationDiagnosticsQuery,
    ReplayProjectionCommand,
};
use rehydration_domain::{GraphNeighborhoodReader, NodeDetailReader};
use rehydration_proto::v1beta1::{
    GetBundleSnapshotRequest, GetBundleSnapshotResponse, GetGraphRelationshipsRequest,
    GetGraphRelationshipsResponse, GetProjectionStatusRequest, GetProjectionStatusResponse,
    GetRehydrationDiagnosticsRequest, GetRehydrationDiagnosticsResponse, Phase,
    ReplayProjectionRequest, ReplayProjectionResponse,
    context_admin_service_server::ContextAdminService,
};
use tonic::{Request, Response, Status};

use crate::transport::proto_mapping_v1beta1::{
    proto_bundle_snapshot_response_v1beta1, proto_graph_relationships_response_v1beta1,
    proto_projection_status_response_v1beta1, proto_rehydration_diagnostics_response_v1beta1,
    proto_replay_projection_response_v1beta1,
};
use crate::transport::support::{map_application_error, map_replay_mode, trim_to_option};

#[derive(Debug, Clone)]
pub struct AdminGrpcServiceV1Beta1<G, D> {
    query_application: Arc<AdminQueryApplicationService<G, D>>,
    command_application: Arc<AdminCommandApplicationService>,
}

impl<G, D> AdminGrpcServiceV1Beta1<G, D> {
    pub fn new(
        query_application: Arc<AdminQueryApplicationService<G, D>>,
        command_application: Arc<AdminCommandApplicationService>,
    ) -> Self {
        Self {
            query_application,
            command_application,
        }
    }
}

#[tonic::async_trait]
impl<G, D> ContextAdminService for AdminGrpcServiceV1Beta1<G, D>
where
    G: GraphNeighborhoodReader + Send + Sync + 'static,
    D: NodeDetailReader + Send + Sync + 'static,
{
    #[tracing::instrument(skip(self, request), fields(rpc = "GetProjectionStatus"))]
    async fn get_projection_status(
        &self,
        request: Request<GetProjectionStatusRequest>,
    ) -> Result<Response<GetProjectionStatusResponse>, Status> {
        let request = request.into_inner();
        let result = self
            .query_application
            .get_projection_status(GetProjectionStatusQuery {
                consumer_names: request.consumer_names,
            });

        Ok(Response::new(proto_projection_status_response_v1beta1(
            &result,
        )))
    }

    #[tracing::instrument(skip(self, request), fields(rpc = "ReplayProjection"))]
    async fn replay_projection(
        &self,
        request: Request<ReplayProjectionRequest>,
    ) -> Result<Response<ReplayProjectionResponse>, Status> {
        let request = request.into_inner();
        let result = self
            .command_application
            .replay_projection(ReplayProjectionCommand {
                consumer_name: request.consumer_name,
                stream_name: request.stream_name,
                starting_after: trim_to_option(request.starting_after),
                max_events: request.max_events,
                replay_mode: map_replay_mode(request.replay_mode),
                requested_by: trim_to_option(request.requested_by),
            })
            .map_err(map_application_error)?;

        Ok(Response::new(proto_replay_projection_response_v1beta1(
            &result,
        )))
    }

    #[tracing::instrument(skip(self, request), fields(rpc = "GetBundleSnapshot"))]
    async fn get_bundle_snapshot(
        &self,
        request: Request<GetBundleSnapshotRequest>,
    ) -> Result<Response<GetBundleSnapshotResponse>, Status> {
        let request = request.into_inner();
        let result = self
            .query_application
            .get_bundle_snapshot(GetBundleSnapshotQuery {
                root_node_id: request.root_node_id,
                role: request.role,
            })
            .await
            .map_err(map_application_error)?;

        Ok(Response::new(proto_bundle_snapshot_response_v1beta1(
            &result,
        )))
    }

    #[tracing::instrument(skip(self, request), fields(rpc = "GetGraphRelationships"))]
    async fn get_graph_relationships(
        &self,
        request: Request<GetGraphRelationshipsRequest>,
    ) -> Result<Response<GetGraphRelationshipsResponse>, Status> {
        let request = request.into_inner();
        let result = self
            .query_application
            .get_graph_relationships(GetGraphRelationshipsQuery {
                node_id: request.node_id,
                node_kind: trim_to_option(request.node_kind),
                depth: request.depth,
                include_reverse_edges: request.include_reverse_edges,
            })
            .await
            .map_err(map_application_error)?;

        Ok(Response::new(proto_graph_relationships_response_v1beta1(
            &result,
        )))
    }

    #[tracing::instrument(skip(self, request), fields(rpc = "GetRehydrationDiagnostics"))]
    async fn get_rehydration_diagnostics(
        &self,
        request: Request<GetRehydrationDiagnosticsRequest>,
    ) -> Result<Response<GetRehydrationDiagnosticsResponse>, Status> {
        let request = request.into_inner();
        let phase = Phase::try_from(request.phase)
            .unwrap_or(Phase::Unspecified)
            .as_str_name()
            .to_string();
        let result = self
            .query_application
            .get_rehydration_diagnostics(GetRehydrationDiagnosticsQuery {
                root_node_id: request.root_node_id,
                roles: request.roles,
                phase: trim_to_option(phase),
            })
            .await
            .map_err(map_application_error)?;

        Ok(Response::new(
            proto_rehydration_diagnostics_response_v1beta1(&result),
        ))
    }
}
