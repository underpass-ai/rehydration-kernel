use std::sync::Arc;

use rehydration_application::{
    AdminQueryApplicationService, CommandApplicationService, QueryApplicationService,
};
use rehydration_domain::{GraphNeighborhoodReader, NodeDetailReader, SnapshotStore};
use rehydration_proto::fleet_context_v1::{
    AddProjectDecisionRequest, AddProjectDecisionResponse, CreateStoryRequest, CreateStoryResponse,
    CreateTaskRequest, CreateTaskResponse, GetContextRequest, GetContextResponse,
    GetGraphRelationshipsRequest, GetGraphRelationshipsResponse, RehydrateSessionRequest,
    RehydrateSessionResponse, TransitionPhaseRequest, TransitionPhaseResponse,
    UpdateContextRequest, UpdateContextResponse, ValidateScopeRequest, ValidateScopeResponse,
    context_service_server::ContextService,
};
use tonic::{Request, Response, Status};

use crate::transport::context_service_compatibility::rpc;

#[derive(Debug, Clone)]
pub struct ContextCompatibilityGrpcService<G, D, S> {
    pub(crate) query_application: Arc<QueryApplicationService<G, D, S>>,
    pub(crate) admin_query_application: Arc<AdminQueryApplicationService<G, D>>,
    pub(crate) command_application: Arc<CommandApplicationService>,
}

impl<G, D, S> ContextCompatibilityGrpcService<G, D, S> {
    pub fn new(
        query_application: Arc<QueryApplicationService<G, D, S>>,
        admin_query_application: Arc<AdminQueryApplicationService<G, D>>,
        command_application: Arc<CommandApplicationService>,
    ) -> Self {
        Self {
            query_application,
            admin_query_application,
            command_application,
        }
    }
}

#[tonic::async_trait]
impl<G, D, S> ContextService for ContextCompatibilityGrpcService<G, D, S>
where
    G: GraphNeighborhoodReader + Send + Sync + 'static,
    D: NodeDetailReader + Send + Sync + 'static,
    S: SnapshotStore + Send + Sync + 'static,
{
    async fn get_context(
        &self,
        request: Request<GetContextRequest>,
    ) -> Result<Response<GetContextResponse>, Status> {
        rpc::get_context::handle(self, request).await
    }

    async fn update_context(
        &self,
        request: Request<UpdateContextRequest>,
    ) -> Result<Response<UpdateContextResponse>, Status> {
        rpc::update_context::handle(self, request).await
    }

    async fn rehydrate_session(
        &self,
        request: Request<RehydrateSessionRequest>,
    ) -> Result<Response<RehydrateSessionResponse>, Status> {
        rpc::rehydrate_session::handle(self, request).await
    }

    async fn validate_scope(
        &self,
        request: Request<ValidateScopeRequest>,
    ) -> Result<Response<ValidateScopeResponse>, Status> {
        rpc::validate_scope::handle(self, request).await
    }

    async fn create_story(
        &self,
        request: Request<CreateStoryRequest>,
    ) -> Result<Response<CreateStoryResponse>, Status> {
        rpc::create_story::handle(request).await
    }

    async fn create_task(
        &self,
        request: Request<CreateTaskRequest>,
    ) -> Result<Response<CreateTaskResponse>, Status> {
        rpc::create_task::handle(request).await
    }

    async fn add_project_decision(
        &self,
        request: Request<AddProjectDecisionRequest>,
    ) -> Result<Response<AddProjectDecisionResponse>, Status> {
        rpc::add_project_decision::handle(request).await
    }

    async fn transition_phase(
        &self,
        request: Request<TransitionPhaseRequest>,
    ) -> Result<Response<TransitionPhaseResponse>, Status> {
        rpc::transition_phase::handle(request).await
    }

    async fn get_graph_relationships(
        &self,
        request: Request<GetGraphRelationshipsRequest>,
    ) -> Result<Response<GetGraphRelationshipsResponse>, Status> {
        rpc::get_graph_relationships::handle(self, request).await
    }
}
