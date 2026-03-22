use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use rehydration_domain::{
    ContextPathNeighborhood, GraphNeighborhoodReader, NodeDetailProjection, NodeDetailReader,
    NodeNeighborhood, PortError, ProcessedEventStore, ProjectionCheckpoint,
    ProjectionCheckpointStore, ProjectionMutation, ProjectionWriter, RehydrationBundle,
    SnapshotSaveOptions, SnapshotStore,
};
use tokio::sync::Mutex;

#[derive(Debug, Default, Clone)]
pub struct InMemoryGraphNeighborhoodReader {
    neighborhoods: HashMap<String, NodeNeighborhood>,
}

impl InMemoryGraphNeighborhoodReader {
    pub fn with_neighborhood(neighborhood: NodeNeighborhood) -> Self {
        let mut neighborhoods = HashMap::new();
        neighborhoods.insert(neighborhood.root.node_id.clone(), neighborhood);
        Self { neighborhoods }
    }
}

impl GraphNeighborhoodReader for InMemoryGraphNeighborhoodReader {
    async fn load_neighborhood(
        &self,
        root_node_id: &str,
        _depth: u32,
    ) -> Result<Option<NodeNeighborhood>, PortError> {
        Ok(self.neighborhoods.get(root_node_id).cloned())
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

#[derive(Debug, Default, Clone)]
pub struct InMemoryNodeDetailReader {
    details: HashMap<String, NodeDetailProjection>,
}

impl InMemoryNodeDetailReader {
    pub fn with_details(details: impl IntoIterator<Item = NodeDetailProjection>) -> Self {
        Self {
            details: details
                .into_iter()
                .map(|detail| (detail.node_id.clone(), detail))
                .collect(),
        }
    }
}

impl NodeDetailReader for InMemoryNodeDetailReader {
    async fn load_node_detail(
        &self,
        node_id: &str,
    ) -> Result<Option<NodeDetailProjection>, PortError> {
        Ok(self.details.get(node_id).cloned())
    }
}

#[derive(Debug, Default, Clone)]
pub struct InMemoryProjectionWriter {
    mutations: Arc<Mutex<Vec<ProjectionMutation>>>,
}

impl InMemoryProjectionWriter {
    pub async fn mutations(&self) -> Vec<ProjectionMutation> {
        self.mutations.lock().await.clone()
    }
}

impl ProjectionWriter for InMemoryProjectionWriter {
    async fn apply_mutations(&self, mutations: Vec<ProjectionMutation>) -> Result<(), PortError> {
        self.mutations.lock().await.extend(mutations);
        Ok(())
    }
}

#[derive(Debug, Default, Clone)]
pub struct InMemoryProcessedEventStore {
    processed: Arc<Mutex<HashSet<(String, String)>>>,
}

impl InMemoryProcessedEventStore {
    pub async fn processed(&self) -> HashSet<(String, String)> {
        self.processed.lock().await.clone()
    }
}

impl ProcessedEventStore for InMemoryProcessedEventStore {
    async fn has_processed(&self, consumer_name: &str, event_id: &str) -> Result<bool, PortError> {
        Ok(self
            .processed
            .lock()
            .await
            .contains(&(consumer_name.to_string(), event_id.to_string())))
    }

    async fn record_processed(&self, consumer_name: &str, event_id: &str) -> Result<(), PortError> {
        self.processed
            .lock()
            .await
            .insert((consumer_name.to_string(), event_id.to_string()));
        Ok(())
    }
}

#[derive(Debug, Default, Clone)]
pub struct InMemoryProjectionCheckpointStore {
    checkpoints: Arc<Mutex<HashMap<(String, String), ProjectionCheckpoint>>>,
}

impl InMemoryProjectionCheckpointStore {
    pub async fn checkpoint(
        &self,
        consumer_name: &str,
        stream_name: &str,
    ) -> Option<ProjectionCheckpoint> {
        self.checkpoints
            .lock()
            .await
            .get(&(consumer_name.to_string(), stream_name.to_string()))
            .cloned()
    }
}

impl ProjectionCheckpointStore for InMemoryProjectionCheckpointStore {
    async fn load_checkpoint(
        &self,
        consumer_name: &str,
        stream_name: &str,
    ) -> Result<Option<ProjectionCheckpoint>, PortError> {
        Ok(self
            .checkpoints
            .lock()
            .await
            .get(&(consumer_name.to_string(), stream_name.to_string()))
            .cloned())
    }

    async fn save_checkpoint(&self, checkpoint: ProjectionCheckpoint) -> Result<(), PortError> {
        let key = (
            checkpoint.consumer_name.clone(),
            checkpoint.stream_name.clone(),
        );
        self.checkpoints.lock().await.insert(key, checkpoint);
        Ok(())
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NoopSnapshotStore;

impl SnapshotStore for NoopSnapshotStore {
    async fn save_bundle_with_options(
        &self,
        _bundle: &RehydrationBundle,
        _options: SnapshotSaveOptions,
    ) -> Result<(), PortError> {
        Ok(())
    }
}

#[cfg(test)]
fn seed_bundle(case_id: rehydration_domain::CaseId, role: &str) -> RehydrationBundle {
    let role = rehydration_domain::Role::new(role).expect("role must be valid");
    RehydrationBundle::new(
        case_id.clone(),
        role.clone(),
        rehydration_domain::BundleNode::new(
            case_id.as_str(),
            "capability",
            format!("Node {}", case_id.as_str()),
            format!(
                "bundle for node {} role {}",
                case_id.as_str(),
                role.as_str()
            ),
            "ACTIVE",
            vec!["projection-node".to_string()],
            std::collections::BTreeMap::new(),
        ),
        Vec::new(),
        Vec::new(),
        vec![rehydration_domain::BundleNodeDetail::new(
            case_id.as_str(),
            format!(
                "bundle for node {} role {}",
                case_id.as_str(),
                role.as_str()
            ),
            "pending",
            1,
        )],
        rehydration_domain::BundleMetadata::initial("0.1.0"),
    )
    .expect("seed bundle should be valid")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use rehydration_domain::{
        CaseId, ContextPathNeighborhood, GraphNeighborhoodReader, NodeDetailProjection,
        NodeDetailReader, NodeNeighborhood, NodeProjection, ProcessedEventStore,
        ProjectionCheckpoint, ProjectionCheckpointStore, ProjectionMutation, ProjectionWriter,
        SnapshotStore,
    };

    use super::{
        InMemoryGraphNeighborhoodReader, InMemoryNodeDetailReader, InMemoryProcessedEventStore,
        InMemoryProjectionCheckpointStore, InMemoryProjectionWriter, NoopSnapshotStore,
        seed_bundle,
    };

    #[tokio::test]
    async fn in_memory_graph_reader_returns_seeded_neighborhood() {
        let reader = InMemoryGraphNeighborhoodReader::with_neighborhood(NodeNeighborhood {
            root: NodeProjection {
                node_id: "node-123".to_string(),
                node_kind: "capability".to_string(),
                title: "Projection".to_string(),
                summary: String::new(),
                status: "ACTIVE".to_string(),
                labels: vec!["projection".to_string()],
                properties: BTreeMap::new(),
            },
            neighbors: Vec::new(),
            relations: Vec::new(),
        });

        let loaded = reader
            .load_neighborhood("node-123", 1)
            .await
            .expect("load should succeed");

        assert!(loaded.is_some());
    }

    #[tokio::test]
    async fn in_memory_node_detail_reader_returns_seeded_detail() {
        let reader = InMemoryNodeDetailReader::with_details([NodeDetailProjection {
            node_id: "node-123".to_string(),
            detail: "Expanded detail".to_string(),
            content_hash: "hash-1".to_string(),
            revision: 1,
        }]);

        let loaded = reader
            .load_node_detail("node-123")
            .await
            .expect("load should succeed");

        assert!(loaded.is_some());
    }

    #[tokio::test]
    async fn in_memory_projection_writer_records_mutations() {
        let writer = InMemoryProjectionWriter::default();
        writer
            .apply_mutations(vec![
                ProjectionMutation::UpsertNode(NodeProjection {
                    node_id: "node-123".to_string(),
                    node_kind: "task".to_string(),
                    title: "Task 123".to_string(),
                    summary: "Projection updated".to_string(),
                    status: "ACTIVE".to_string(),
                    labels: vec!["work-item".to_string()],
                    properties: BTreeMap::new(),
                }),
                ProjectionMutation::UpsertNodeDetail(NodeDetailProjection {
                    node_id: "node-123".to_string(),
                    detail: "expanded node detail".to_string(),
                    content_hash: "hash-1".to_string(),
                    revision: 1,
                }),
            ])
            .await
            .expect("write should succeed");

        assert_eq!(writer.mutations().await.len(), 2);
    }

    #[tokio::test]
    async fn in_memory_processed_event_store_tracks_deduplication() {
        let store = InMemoryProcessedEventStore::default();
        assert!(
            !store
                .has_processed("context-projection", "evt-1")
                .await
                .expect("lookup should succeed")
        );

        store
            .record_processed("context-projection", "evt-1")
            .await
            .expect("record should succeed");

        assert!(
            store
                .has_processed("context-projection", "evt-1")
                .await
                .expect("lookup should succeed")
        );
        assert_eq!(store.processed().await.len(), 1);
    }

    #[tokio::test]
    async fn in_memory_checkpoint_store_persists_latest_checkpoint() {
        let store = InMemoryProjectionCheckpointStore::default();
        store
            .save_checkpoint(ProjectionCheckpoint {
                consumer_name: "context-projection".to_string(),
                stream_name: "rehydration.events".to_string(),
                last_subject: "graph.node.materialized".to_string(),
                last_event_id: "evt-1".to_string(),
                last_correlation_id: "corr-1".to_string(),
                last_occurred_at: "2026-03-07T00:00:00Z".to_string(),
                processed_events: 1,
                updated_at: std::time::SystemTime::UNIX_EPOCH,
            })
            .await
            .expect("save should succeed");

        let checkpoint = store
            .checkpoint("context-projection", "rehydration.events")
            .await
            .expect("checkpoint should exist");
        assert_eq!(checkpoint.last_event_id, "evt-1");
    }

    #[tokio::test]
    async fn noop_snapshot_store_accepts_bundle() {
        let bundle = seed_bundle(
            CaseId::new("case-123").expect("case id is valid"),
            "developer",
        );

        NoopSnapshotStore
            .save_bundle(&bundle)
            .await
            .expect("noop snapshot store should accept bundles");
    }
}
