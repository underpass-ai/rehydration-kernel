use std::time::Duration;

use rehydration_domain::{GraphNeighborhoodReader, NodeDetailReader, RehydrationBundle};

use crate::ApplicationError;
use crate::queries::{AdminQueryApplicationService, BundleAssembler, NodeCentricProjectionReader};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetBundleSnapshotQuery {
    pub root_node_id: String,
    pub role: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleSnapshotResult {
    pub snapshot_id: String,
    pub root_node_id: String,
    pub role: String,
    pub bundle: RehydrationBundle,
    pub created_at: std::time::SystemTime,
    pub expires_at: std::time::SystemTime,
    pub ttl_seconds: u64,
}

#[derive(Debug)]
pub struct GetBundleSnapshotUseCase<G, D> {
    graph_reader: G,
    detail_reader: D,
    generator_version: &'static str,
}

impl<G, D> GetBundleSnapshotUseCase<G, D>
where
    G: GraphNeighborhoodReader + Send + Sync,
    D: NodeDetailReader + Send + Sync,
{
    pub fn new(graph_reader: G, detail_reader: D, generator_version: &'static str) -> Self {
        Self {
            graph_reader,
            detail_reader,
            generator_version,
        }
    }

    pub async fn execute(
        &self,
        query: GetBundleSnapshotQuery,
    ) -> Result<BundleSnapshotResult, ApplicationError> {
        let bundle = self
            .load_or_placeholder_bundle(&query.root_node_id, &query.role)
            .await?;
        let created_at = std::time::SystemTime::now();
        let ttl_seconds = 900;
        let expires_at = created_at
            .checked_add(Duration::from_secs(ttl_seconds))
            .unwrap_or(created_at);

        Ok(BundleSnapshotResult {
            snapshot_id: format!(
                "snapshot:{}:{}",
                bundle.root_node_id().as_str(),
                bundle.role().as_str()
            ),
            root_node_id: bundle.root_node_id().as_str().to_string(),
            role: bundle.role().as_str().to_string(),
            bundle,
            created_at,
            expires_at,
            ttl_seconds,
        })
    }

    async fn load_or_placeholder_bundle(
        &self,
        root_node_id: &str,
        role: &str,
    ) -> Result<RehydrationBundle, ApplicationError> {
        let bundle_reader =
            NodeCentricProjectionReader::new(&self.graph_reader, &self.detail_reader);
        match bundle_reader
            .load_bundle(root_node_id, role, self.generator_version)
            .await?
        {
            Some(bundle) => Ok(bundle),
            None => BundleAssembler::placeholder(root_node_id, role, self.generator_version),
        }
    }
}

impl<G, D> AdminQueryApplicationService<G, D>
where
    G: GraphNeighborhoodReader + Send + Sync,
    D: NodeDetailReader + Send + Sync,
{
    pub async fn get_bundle_snapshot(
        &self,
        query: GetBundleSnapshotQuery,
    ) -> Result<BundleSnapshotResult, ApplicationError> {
        GetBundleSnapshotUseCase::new(
            std::sync::Arc::clone(&self.graph_reader),
            std::sync::Arc::clone(&self.detail_reader),
            self.generator_version,
        )
        .execute(query)
        .await
    }
}
