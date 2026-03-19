use std::sync::Arc;

use rehydration_application::{
    ContextRenderOptions, GetContextQuery, GetNodeDetailQuery, QueryApplicationService,
    RehydrateSessionQuery, ValidateScopeQuery,
};
use rehydration_domain::{GraphNeighborhoodReader, NodeDetailReader, SnapshotStore};
use rehydration_proto::v1alpha1::{
    GetContextRequest, GetContextResponse, GetNodeDetailRequest, GetNodeDetailResponse,
    RehydrateSessionRequest, RehydrateSessionResponse, ValidateScopeRequest, ValidateScopeResponse,
    context_query_service_server::ContextQueryService,
};
use tonic::{Request, Response, Status};

use crate::transport::proto_mapping::{
    proto_bundle_from_single_role, proto_graph_node, proto_node_detail_view,
    proto_rehydrate_session_response, proto_rendered_context_from_result, proto_scope_validation,
};
use crate::transport::support::map_application_error;

#[derive(Debug, Clone)]
pub struct QueryGrpcService<G, D, S> {
    application: Arc<QueryApplicationService<G, D, S>>,
}

impl<G, D, S> QueryGrpcService<G, D, S> {
    pub fn new(application: Arc<QueryApplicationService<G, D, S>>) -> Self {
        Self { application }
    }
}

#[tonic::async_trait]
impl<G, D, S> ContextQueryService for QueryGrpcService<G, D, S>
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
            bundle: Some(proto_bundle_from_single_role(&result.bundle)),
            rendered: Some(proto_rendered_context_from_result(&result)),
            scope_validation: Some(proto_scope_validation(&result.scope_validation)),
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
            node: Some(proto_graph_node(&result.node)),
            detail: result.detail.as_ref().map(proto_node_detail_view),
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

        Ok(Response::new(proto_rehydrate_session_response(&result)))
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
            result: Some(proto_scope_validation(&result)),
        }))
    }
}
