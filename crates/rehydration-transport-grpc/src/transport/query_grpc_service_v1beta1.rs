use std::sync::Arc;

use rehydration_application::{
    ContextRenderOptions, GetContextPathQuery, GetContextQuery, GetNodeDetailQuery,
    QueryApplicationService, RehydrateSessionQuery, ValidateScopeQuery,
};
use rehydration_domain::{GraphNeighborhoodReader, NodeDetailReader, SnapshotStore};
use rehydration_proto::v1beta1::{
    GetContextPathRequest, GetContextPathResponse, GetContextRequest, GetContextResponse,
    GetNodeDetailRequest, GetNodeDetailResponse, RehydrateSessionRequest, RehydrateSessionResponse,
    ValidateScopeRequest, ValidateScopeResponse, context_query_service_server::ContextQueryService,
};
use tonic::{Request, Response, Status};

use crate::transport::proto_mapping_v1beta1::{
    proto_bundle_from_single_role_v1beta1, proto_graph_node_v1beta1,
    proto_node_detail_view_v1beta1, proto_rehydrate_session_response_v1beta1,
    proto_rendered_context_from_result_v1beta1, proto_rendered_context_v1beta1,
    proto_scope_validation_v1beta1,
};
use crate::transport::support::map_application_error;

#[derive(Debug, Clone)]
pub struct QueryGrpcServiceV1Beta1<G, D, S> {
    application: Arc<QueryApplicationService<G, D, S>>,
}

impl<G, D, S> QueryGrpcServiceV1Beta1<G, D, S> {
    pub fn new(application: Arc<QueryApplicationService<G, D, S>>) -> Self {
        Self { application }
    }
}

#[tonic::async_trait]
impl<G, D, S> ContextQueryService for QueryGrpcServiceV1Beta1<G, D, S>
where
    G: GraphNeighborhoodReader + Send + Sync + 'static,
    D: NodeDetailReader + Send + Sync + 'static,
    S: SnapshotStore + Send + Sync + 'static,
{
    async fn get_context(
        &self,
        request: Request<GetContextRequest>,
    ) -> Result<Response<GetContextResponse>, Status> {
        let request = request.into_inner();
        let result = self
            .application
            .get_context(GetContextQuery {
                root_node_id: request.root_node_id,
                role: request.role,
                depth: request.depth,
                render_options: ContextRenderOptions {
                    focus_node_id: None,
                    token_budget: (request.token_budget > 0).then_some(request.token_budget),
                },
                requested_scopes: request.requested_scopes,
            })
            .await
            .map_err(map_application_error)?;

        Ok(Response::new(GetContextResponse {
            bundle: Some(proto_bundle_from_single_role_v1beta1(&result.bundle)),
            rendered: Some(proto_rendered_context_from_result_v1beta1(&result)),
            scope_validation: Some(proto_scope_validation_v1beta1(&result.scope_validation)),
            served_at: Some(crate::transport::support::timestamp_from(result.served_at)),
        }))
    }

    async fn get_context_path(
        &self,
        request: Request<GetContextPathRequest>,
    ) -> Result<Response<GetContextPathResponse>, Status> {
        let request = request.into_inner();
        let result = self
            .application
            .get_context_path(GetContextPathQuery {
                root_node_id: request.root_node_id,
                target_node_id: request.target_node_id,
                role: request.role,
                render_options: ContextRenderOptions {
                    focus_node_id: None,
                    token_budget: (request.token_budget > 0).then_some(request.token_budget),
                },
            })
            .await
            .map_err(map_application_error)?;

        Ok(Response::new(GetContextPathResponse {
            path_bundle: Some(proto_bundle_from_single_role_v1beta1(&result.path_bundle)),
            rendered: Some(proto_rendered_context_v1beta1(&result.rendered, &[])),
            served_at: Some(crate::transport::support::timestamp_from(result.served_at)),
        }))
    }

    async fn get_node_detail(
        &self,
        request: Request<GetNodeDetailRequest>,
    ) -> Result<Response<GetNodeDetailResponse>, Status> {
        let request = request.into_inner();
        let result = self
            .application
            .get_node_detail(GetNodeDetailQuery {
                node_id: request.node_id,
            })
            .await
            .map_err(map_application_error)?;

        Ok(Response::new(GetNodeDetailResponse {
            node: Some(proto_graph_node_v1beta1(&result.node)),
            detail: result.detail.as_ref().map(proto_node_detail_view_v1beta1),
        }))
    }

    async fn rehydrate_session(
        &self,
        request: Request<RehydrateSessionRequest>,
    ) -> Result<Response<RehydrateSessionResponse>, Status> {
        let request = request.into_inner();
        let result = self
            .application
            .rehydrate_session(RehydrateSessionQuery {
                root_node_id: request.root_node_id,
                roles: request.roles,
                persist_snapshot: request.persist_snapshot,
                snapshot_ttl_seconds: 900,
                timeline_window: request.timeline_window,
            })
            .await
            .map_err(map_application_error)?;

        Ok(Response::new(proto_rehydrate_session_response_v1beta1(
            &result,
        )))
    }

    async fn validate_scope(
        &self,
        request: Request<ValidateScopeRequest>,
    ) -> Result<Response<ValidateScopeResponse>, Status> {
        let request = request.into_inner();
        let result = self.application.validate_scope(ValidateScopeQuery {
            required_scopes: request.required_scopes,
            provided_scopes: request.provided_scopes,
        });

        Ok(Response::new(ValidateScopeResponse {
            result: Some(proto_scope_validation_v1beta1(&result)),
        }))
    }
}
