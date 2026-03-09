use std::sync::Arc;

use rehydration_application::{
    ApplicationError, CommandApplicationService, QueryApplicationService, RehydrateSessionQuery,
    RehydrateSessionResult, UpdateContextCommand, UpdateContextOutcome,
};
use rehydration_domain::{GraphNeighborhoodReader, NodeDetailReader, SnapshotStore};

use crate::compatibility::ContextAsyncService;

#[derive(Debug)]
pub struct ContextAsyncApplication<G, D, S> {
    command_application: Arc<CommandApplicationService>,
    query_application: Arc<QueryApplicationService<G, D, S>>,
}

impl<G, D, S> ContextAsyncApplication<G, D, S> {
    pub fn new(
        command_application: Arc<CommandApplicationService>,
        query_application: Arc<QueryApplicationService<G, D, S>>,
    ) -> Self {
        Self {
            command_application,
            query_application,
        }
    }
}

impl<G, D, S> ContextAsyncService for ContextAsyncApplication<G, D, S>
where
    G: GraphNeighborhoodReader + Send + Sync,
    D: NodeDetailReader + Send + Sync,
    S: SnapshotStore + Send + Sync,
{
    async fn update_context(
        &self,
        command: UpdateContextCommand,
    ) -> Result<UpdateContextOutcome, ApplicationError> {
        self.command_application.update_context(command)
    }

    async fn rehydrate_session(
        &self,
        query: RehydrateSessionQuery,
    ) -> Result<RehydrateSessionResult, ApplicationError> {
        self.query_application.rehydrate_session(query).await
    }
}
