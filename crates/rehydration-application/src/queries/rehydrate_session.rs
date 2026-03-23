use std::sync::Arc;
use std::time::SystemTime;

use rehydration_domain::{
    BundleMetadata, GraphNeighborhoodReader, NodeDetailReader, RehydrationBundle,
    SnapshotSaveOptions, SnapshotStore,
};

use crate::ApplicationError;
use crate::queries::{NodeCentricProjectionReader, QueryApplicationService};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RehydrateSessionQuery {
    pub root_node_id: String,
    pub roles: Vec<String>,
    pub persist_snapshot: bool,
    pub timeline_window: u32,
    pub snapshot_ttl_seconds: u64,
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
        snapshot_options: SnapshotSaveOptions,
    ) -> Result<RehydrationBundle, ApplicationError> {
        self.execute_with_depth(
            root_node_id,
            role,
            crate::queries::DEFAULT_NATIVE_GRAPH_TRAVERSAL_DEPTH,
            persist_snapshot,
            snapshot_options,
        )
        .await
    }

    pub async fn execute_with_depth(
        &self,
        root_node_id: &str,
        role: &str,
        depth: u32,
        persist_snapshot: bool,
        snapshot_options: SnapshotSaveOptions,
    ) -> Result<RehydrationBundle, ApplicationError> {
        let bundle_reader =
            NodeCentricProjectionReader::new(&self.graph_reader, &self.detail_reader);
        let bundle = match bundle_reader
            .load_bundle_with_depth(root_node_id, role, self.generator_version, depth)
            .await?
        {
            Some(bundle) => bundle,
            None => {
                return Err(ApplicationError::NotFound(format!(
                    "node '{}' not found",
                    root_node_id
                )));
            }
        };

        if persist_snapshot {
            self.snapshot_store
                .save_bundle_with_options(&bundle, snapshot_options)
                .await?;
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
        let snapshot_options = SnapshotSaveOptions::new(Some(query.snapshot_ttl_seconds));

        let mut bundles = Vec::with_capacity(query.roles.len());
        for role in &query.roles {
            bundles.push(
                use_case
                    .execute(
                        &query.root_node_id,
                        role,
                        query.persist_snapshot,
                        snapshot_options,
                    )
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
        .execute(
            "bootstrap-node",
            "system",
            false,
            SnapshotSaveOptions::default(),
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use tokio::sync::Mutex;

    use rehydration_domain::{
        ContextPathNeighborhood, NodeDetailProjection, NodeNeighborhood, NodeProjection, PortError,
        SnapshotSaveOptions,
    };

    use super::{QueryApplicationService, RehydrateSessionQuery};

    struct SeededGraphReader;

    impl rehydration_domain::GraphNeighborhoodReader for SeededGraphReader {
        async fn load_neighborhood(
            &self,
            root_node_id: &str,
            _depth: u32,
        ) -> Result<Option<NodeNeighborhood>, PortError> {
            Ok(Some(NodeNeighborhood {
                root: NodeProjection {
                    node_id: root_node_id.to_string(),
                    node_kind: "story".to_string(),
                    title: "Root".to_string(),
                    summary: "Root summary".to_string(),
                    status: "ACTIVE".to_string(),
                    labels: vec!["Story".to_string()],
                    properties: BTreeMap::new(),
                },
                neighbors: Vec::new(),
                relations: Vec::new(),
            }))
        }

        async fn load_context_path(
            &self,
            _root_node_id: &str,
            _target_node_id: &str,
            _subtree_depth: u32,
        ) -> Result<Option<ContextPathNeighborhood>, PortError> {
            Ok(None)
        }
    }

    struct SeededDetailReader;

    impl rehydration_domain::NodeDetailReader for SeededDetailReader {
        async fn load_node_detail(
            &self,
            node_id: &str,
        ) -> Result<Option<NodeDetailProjection>, PortError> {
            Ok(Some(NodeDetailProjection {
                node_id: node_id.to_string(),
                detail: "Expanded detail".to_string(),
                content_hash: "hash-1".to_string(),
                revision: 2,
            }))
        }
    }

    #[derive(Debug, Default)]
    struct RecordingSnapshotStore {
        options: Mutex<Vec<SnapshotSaveOptions>>,
    }

    impl rehydration_domain::SnapshotStore for RecordingSnapshotStore {
        async fn save_bundle_with_options(
            &self,
            _bundle: &rehydration_domain::RehydrationBundle,
            options: SnapshotSaveOptions,
        ) -> Result<(), PortError> {
            self.options.lock().await.push(options);
            Ok(())
        }
    }

    #[tokio::test]
    async fn rehydrate_session_propagates_snapshot_ttl_to_store() {
        let snapshot_store = Arc::new(RecordingSnapshotStore::default());
        let service = QueryApplicationService::new(
            Arc::new(SeededGraphReader),
            Arc::new(SeededDetailReader),
            Arc::clone(&snapshot_store),
            "0.1.0",
        );

        let result = service
            .rehydrate_session(RehydrateSessionQuery {
                root_node_id: "story-123".to_string(),
                roles: vec!["developer".to_string()],
                persist_snapshot: true,
                timeline_window: 50,
                snapshot_ttl_seconds: 1800,
            })
            .await
            .expect("rehydration should succeed");

        assert!(result.snapshot_persisted);
        assert_eq!(
            snapshot_store.options.lock().await.as_slice(),
            &[SnapshotSaveOptions::new(Some(1800))]
        );
    }

    struct EmptyGraphReader;

    impl rehydration_domain::GraphNeighborhoodReader for EmptyGraphReader {
        async fn load_neighborhood(
            &self,
            _root_node_id: &str,
            _depth: u32,
        ) -> Result<Option<NodeNeighborhood>, PortError> {
            Ok(None)
        }

        async fn load_context_path(
            &self,
            _root_node_id: &str,
            _target_node_id: &str,
            _subtree_depth: u32,
        ) -> Result<Option<ContextPathNeighborhood>, PortError> {
            Ok(None)
        }
    }

    #[tokio::test]
    async fn rehydrate_session_returns_not_found_when_node_does_not_exist() {
        let service = QueryApplicationService::new(
            Arc::new(EmptyGraphReader),
            Arc::new(SeededDetailReader),
            Arc::new(RecordingSnapshotStore::default()),
            "0.1.0",
        );

        let result = service
            .rehydrate_session(RehydrateSessionQuery {
                root_node_id: "nonexistent-node".to_string(),
                roles: vec!["developer".to_string()],
                persist_snapshot: false,
                timeline_window: 0,
                snapshot_ttl_seconds: 0,
            })
            .await;

        assert!(result.is_err(), "should return an error for missing node");
        let err = result.unwrap_err();
        assert!(
            matches!(err, crate::ApplicationError::NotFound(_)),
            "error should be NotFound, got: {err:?}"
        );
    }
}
