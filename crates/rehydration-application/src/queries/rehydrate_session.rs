use std::sync::Arc;
use std::time::SystemTime;

use rehydration_domain::{
    BundleMetadata, GraphNeighborhoodReader, NodeDetailReader, RehydrationBundle, SnapshotStore,
};

use crate::ApplicationError;
use crate::queries::{BundleAssembler, NodeCentricProjectionReader, QueryApplicationService};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RehydrateSessionQuery {
    pub root_node_id: String,
    pub roles: Vec<String>,
    pub persist_snapshot: bool,
    pub timeline_window: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RehydrateSessionResult {
    pub root_node_id: String,
    pub bundles: Vec<RehydrationBundle>,
    pub timeline_events: u32,
    pub version: BundleMetadata,
    pub snapshot_persisted: bool,
    pub snapshot_id: Option<String>,
    pub generated_at: SystemTime,
}

#[derive(Debug)]
pub struct RehydrateSessionUseCase<G, D, S> {
    graph_reader: G,
    detail_reader: D,
    snapshot_store: S,
    generator_version: &'static str,
}

impl<G, D, S> RehydrateSessionUseCase<G, D, S>
where
    G: GraphNeighborhoodReader + Send + Sync,
    D: NodeDetailReader + Send + Sync,
    S: SnapshotStore + Send + Sync,
{
    pub fn new(
        graph_reader: G,
        detail_reader: D,
        snapshot_store: S,
        generator_version: &'static str,
    ) -> Self {
        Self {
            graph_reader,
            detail_reader,
            snapshot_store,
            generator_version,
        }
    }

    pub async fn execute(
        &self,
        root_node_id: &str,
        role: &str,
        persist_snapshot: bool,
    ) -> Result<RehydrationBundle, ApplicationError> {
        let pack_reader = NodeCentricProjectionReader::new(&self.graph_reader, &self.detail_reader);
        let bundle = match pack_reader.load_pack(root_node_id, role).await? {
            Some(pack) => BundleAssembler::assemble(pack, self.generator_version),
            None => BundleAssembler::placeholder(root_node_id, role, self.generator_version)?,
        };

        if persist_snapshot {
            self.snapshot_store.save_bundle(&bundle).await?;
        }
        Ok(bundle)
    }
}

impl<G, D, S> QueryApplicationService<G, D, S>
where
    G: GraphNeighborhoodReader + Send + Sync,
    D: NodeDetailReader + Send + Sync,
    S: SnapshotStore + Send + Sync,
{
    pub async fn rehydrate_session(
        &self,
        query: RehydrateSessionQuery,
    ) -> Result<RehydrateSessionResult, ApplicationError> {
        if query.roles.is_empty() {
            return Err(ApplicationError::Validation(
                "roles cannot be empty".to_string(),
            ));
        }

        let use_case = RehydrateSessionUseCase::new(
            Arc::clone(&self.graph_reader),
            Arc::clone(&self.detail_reader),
            Arc::clone(&self.snapshot_store),
            self.generator_version,
        );

        let mut bundles = Vec::with_capacity(query.roles.len());
        for role in &query.roles {
            bundles.push(
                use_case
                    .execute(&query.root_node_id, role, query.persist_snapshot)
                    .await?,
            );
        }

        let snapshot_id = if query.persist_snapshot {
            Some(format!(
                "snapshot:{}:{}",
                query.root_node_id,
                query.roles.join(",")
            ))
        } else {
            None
        };

        Ok(RehydrateSessionResult {
            root_node_id: query.root_node_id,
            bundles,
            timeline_events: query.timeline_window,
            version: BundleMetadata::initial(self.generator_version),
            snapshot_persisted: query.persist_snapshot,
            snapshot_id,
            generated_at: SystemTime::now(),
        })
    }

    pub async fn warmup_bundle(&self) -> Result<RehydrationBundle, ApplicationError> {
        RehydrateSessionUseCase::new(
            Arc::clone(&self.graph_reader),
            Arc::clone(&self.detail_reader),
            Arc::clone(&self.snapshot_store),
            self.generator_version,
        )
        .execute("bootstrap-case", "system", false)
        .await
    }
}
