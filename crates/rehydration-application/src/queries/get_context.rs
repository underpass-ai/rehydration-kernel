use rehydration_domain::{
    GraphNeighborhoodReader, NodeDetailReader, RehydrationBundle, SnapshotStore,
};

use crate::ApplicationError;
pub use crate::queries::render_graph_bundle::RenderedContext;
use crate::queries::{
    QueryApplicationService, RehydrateSessionUseCase, ScopeValidation, ValidateScopeUseCase,
    render_graph_bundle,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetContextQuery {
    pub root_node_id: String,
    pub role: String,
    pub requested_scopes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetContextResult {
    pub bundle: RehydrationBundle,
    pub rendered: RenderedContext,
    pub scope_validation: ScopeValidation,
    pub served_at: std::time::SystemTime,
}

#[derive(Debug)]
pub struct GetContextUseCase<G, D, S> {
    rehydrate_session: RehydrateSessionUseCase<G, D, S>,
}

impl<G, D, S> GetContextUseCase<G, D, S>
where
    G: GraphNeighborhoodReader + Send + Sync,
    D: NodeDetailReader + Send + Sync,
    S: SnapshotStore + Send + Sync,
{
    pub fn new(rehydrate_session: RehydrateSessionUseCase<G, D, S>) -> Self {
        Self { rehydrate_session }
    }

    pub async fn execute(
        &self,
        root_node_id: &str,
        role: &str,
        requested_scopes: &[String],
    ) -> Result<GetContextResult, ApplicationError> {
        let bundle = self
            .rehydrate_session
            .execute(root_node_id, role, false)
            .await?;
        let rendered = render_graph_bundle(&bundle);
        let scope_validation = ValidateScopeUseCase::execute(requested_scopes, requested_scopes);

        Ok(GetContextResult {
            bundle,
            rendered,
            scope_validation,
            served_at: std::time::SystemTime::now(),
        })
    }
}

impl<G, D, S> QueryApplicationService<G, D, S>
where
    G: GraphNeighborhoodReader + Send + Sync,
    D: NodeDetailReader + Send + Sync,
    S: SnapshotStore + Send + Sync,
{
    pub async fn get_context(
        &self,
        query: GetContextQuery,
    ) -> Result<GetContextResult, ApplicationError> {
        let rehydrate = RehydrateSessionUseCase::new(
            std::sync::Arc::clone(&self.graph_reader),
            std::sync::Arc::clone(&self.detail_reader),
            std::sync::Arc::clone(&self.snapshot_store),
            self.generator_version,
        );

        GetContextUseCase::new(rehydrate)
            .execute(&query.root_node_id, &query.role, &query.requested_scopes)
            .await
    }
}
