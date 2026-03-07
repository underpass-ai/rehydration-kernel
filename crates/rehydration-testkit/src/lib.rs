use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use rehydration_domain::{CaseId, RehydrationBundle, Role, RoleContextPack};
use rehydration_ports::{
    PortError, ProcessedEventStore, ProjectionCheckpoint, ProjectionCheckpointStore,
    ProjectionMutation, ProjectionReader, ProjectionWriter, SnapshotStore,
};
use tokio::sync::Mutex;

#[derive(Debug, Default, Clone)]
pub struct InMemoryProjectionReader {
    packs: HashMap<(CaseId, Role), RoleContextPack>,
}

impl InMemoryProjectionReader {
    pub fn with_pack(pack: RoleContextPack) -> Self {
        let key = (pack.case_header().case_id().clone(), pack.role().clone());
        let mut packs = HashMap::new();
        packs.insert(key, pack);
        Self { packs }
    }

    pub fn with_bundle(bundle: RehydrationBundle) -> Self {
        Self::with_pack(bundle.pack().clone())
    }
}

impl ProjectionReader for InMemoryProjectionReader {
    async fn load_pack(
        &self,
        case_id: &CaseId,
        role: &Role,
    ) -> Result<Option<RoleContextPack>, PortError> {
        Ok(self.packs.get(&(case_id.clone(), role.clone())).cloned())
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
    async fn save_bundle(&self, _bundle: &RehydrationBundle) -> Result<(), PortError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use rehydration_domain::{CaseId, RehydrationBundle, Role};
    use rehydration_ports::{
        NodeDetailProjection, NodeProjection, ProcessedEventStore, ProjectionCheckpoint,
        ProjectionCheckpointStore, ProjectionMutation, ProjectionReader, ProjectionWriter,
        SnapshotStore,
    };

    use super::{
        InMemoryProcessedEventStore, InMemoryProjectionCheckpointStore, InMemoryProjectionReader,
        InMemoryProjectionWriter, NoopSnapshotStore,
    };

    #[tokio::test]
    async fn in_memory_reader_returns_seeded_bundle() {
        let bundle = RehydrationBundle::empty(
            CaseId::new("case-123").expect("case id is valid"),
            Role::new("developer").expect("role is valid"),
            "0.1.0",
        );
        let reader = InMemoryProjectionReader::with_bundle(bundle);

        let loaded = reader
            .load_pack(
                &CaseId::new("case-123").expect("case id is valid"),
                &Role::new("developer").expect("role is valid"),
            )
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
        let bundle = RehydrationBundle::empty(
            CaseId::new("case-123").expect("case id is valid"),
            Role::new("developer").expect("role is valid"),
            "0.1.0",
        );

        NoopSnapshotStore
            .save_bundle(&bundle)
            .await
            .expect("noop snapshot store should accept bundles");
    }
}
