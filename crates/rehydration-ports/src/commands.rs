use std::future::Future;
use std::sync::Arc;
use std::time::SystemTime;

use rehydration_domain::RehydrationBundle;

use crate::PortError;
use crate::queries::{NodeDetailProjection, NodeProjection, NodeRelationProjection};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectionMutation {
    UpsertNode(NodeProjection),
    UpsertNodeRelation(NodeRelationProjection),
    UpsertNodeDetail(NodeDetailProjection),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionCheckpoint {
    pub consumer_name: String,
    pub stream_name: String,
    pub last_subject: String,
    pub last_event_id: String,
    pub last_correlation_id: String,
    pub last_occurred_at: String,
    pub processed_events: u64,
    pub updated_at: SystemTime,
}

pub trait ProjectionWriter {
    fn apply_mutations(
        &self,
        mutations: Vec<ProjectionMutation>,
    ) -> impl Future<Output = Result<(), PortError>> + Send;
}

pub trait ProcessedEventStore {
    fn has_processed(
        &self,
        consumer_name: &str,
        event_id: &str,
    ) -> impl Future<Output = Result<bool, PortError>> + Send;

    fn record_processed(
        &self,
        consumer_name: &str,
        event_id: &str,
    ) -> impl Future<Output = Result<(), PortError>> + Send;
}

pub trait ProjectionCheckpointStore {
    fn load_checkpoint(
        &self,
        consumer_name: &str,
        stream_name: &str,
    ) -> impl Future<Output = Result<Option<ProjectionCheckpoint>, PortError>> + Send;

    fn save_checkpoint(
        &self,
        checkpoint: ProjectionCheckpoint,
    ) -> impl Future<Output = Result<(), PortError>> + Send;
}

pub trait SnapshotStore {
    fn save_bundle(
        &self,
        bundle: &RehydrationBundle,
    ) -> impl Future<Output = Result<(), PortError>> + Send;
}

impl<T> ProjectionWriter for Arc<T>
where
    T: ProjectionWriter + Send + Sync + ?Sized,
{
    async fn apply_mutations(&self, mutations: Vec<ProjectionMutation>) -> Result<(), PortError> {
        self.as_ref().apply_mutations(mutations).await
    }
}

impl<T> ProcessedEventStore for Arc<T>
where
    T: ProcessedEventStore + Send + Sync + ?Sized,
{
    async fn has_processed(&self, consumer_name: &str, event_id: &str) -> Result<bool, PortError> {
        self.as_ref().has_processed(consumer_name, event_id).await
    }

    async fn record_processed(&self, consumer_name: &str, event_id: &str) -> Result<(), PortError> {
        self.as_ref()
            .record_processed(consumer_name, event_id)
            .await
    }
}

impl<T> ProjectionCheckpointStore for Arc<T>
where
    T: ProjectionCheckpointStore + Send + Sync + ?Sized,
{
    async fn load_checkpoint(
        &self,
        consumer_name: &str,
        stream_name: &str,
    ) -> Result<Option<ProjectionCheckpoint>, PortError> {
        self.as_ref()
            .load_checkpoint(consumer_name, stream_name)
            .await
    }

    async fn save_checkpoint(&self, checkpoint: ProjectionCheckpoint) -> Result<(), PortError> {
        self.as_ref().save_checkpoint(checkpoint).await
    }
}

impl<T> SnapshotStore for Arc<T>
where
    T: SnapshotStore + Send + Sync + ?Sized,
{
    async fn save_bundle(&self, bundle: &RehydrationBundle) -> Result<(), PortError> {
        self.as_ref().save_bundle(bundle).await
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;
    use std::time::SystemTime;

    use rehydration_domain::{CaseId, RehydrationBundle, Role};

    use super::{
        ProcessedEventStore, ProjectionCheckpoint, ProjectionCheckpointStore, ProjectionMutation,
        ProjectionWriter, SnapshotStore,
    };
    use crate::PortError;
    use crate::queries::{NodeDetailProjection, NodeProjection};

    struct Writer;

    impl ProjectionWriter for Writer {
        async fn apply_mutations(
            &self,
            mutations: Vec<ProjectionMutation>,
        ) -> Result<(), PortError> {
            assert_eq!(mutations.len(), 1);
            Ok(())
        }
    }

    struct EventStore;

    impl ProcessedEventStore for EventStore {
        async fn has_processed(
            &self,
            _consumer_name: &str,
            event_id: &str,
        ) -> Result<bool, PortError> {
            Ok(event_id == "evt-1")
        }

        async fn record_processed(
            &self,
            _consumer_name: &str,
            _event_id: &str,
        ) -> Result<(), PortError> {
            Ok(())
        }
    }

    struct CheckpointStore;

    impl ProjectionCheckpointStore for CheckpointStore {
        async fn load_checkpoint(
            &self,
            consumer_name: &str,
            stream_name: &str,
        ) -> Result<Option<ProjectionCheckpoint>, PortError> {
            Ok(Some(ProjectionCheckpoint {
                consumer_name: consumer_name.to_string(),
                stream_name: stream_name.to_string(),
                last_subject: "graph.node.materialized".to_string(),
                last_event_id: "evt-1".to_string(),
                last_correlation_id: "corr-1".to_string(),
                last_occurred_at: "2026-03-07T00:00:00Z".to_string(),
                processed_events: 1,
                updated_at: SystemTime::UNIX_EPOCH,
            }))
        }

        async fn save_checkpoint(
            &self,
            _checkpoint: ProjectionCheckpoint,
        ) -> Result<(), PortError> {
            Ok(())
        }
    }

    struct SnapshotWriter;

    impl SnapshotStore for SnapshotWriter {
        async fn save_bundle(&self, _bundle: &RehydrationBundle) -> Result<(), PortError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn projection_writer_delegates_through_arc() {
        let writer = Arc::new(Writer);
        writer
            .apply_mutations(vec![ProjectionMutation::UpsertNode(NodeProjection {
                node_id: "node-123".to_string(),
                node_kind: "capability".to_string(),
                title: "Projection".to_string(),
                summary: String::new(),
                status: "ACTIVE".to_string(),
                labels: vec!["projection".to_string()],
                properties: BTreeMap::new(),
            })])
            .await
            .expect("write should succeed");
    }

    #[tokio::test]
    async fn processed_event_store_delegates_through_arc() {
        let store = Arc::new(EventStore);
        assert!(
            store
                .has_processed("context-projection", "evt-1")
                .await
                .expect("lookup should succeed")
        );
    }

    #[tokio::test]
    async fn checkpoint_store_delegates_through_arc() {
        let store = Arc::new(CheckpointStore);
        let checkpoint = store
            .load_checkpoint("context-projection", "rehydration.events")
            .await
            .expect("load should succeed")
            .expect("checkpoint should exist");

        assert_eq!(checkpoint.last_event_id, "evt-1");
    }

    #[tokio::test]
    async fn snapshot_store_delegates_through_arc() {
        let store = Arc::new(SnapshotWriter);
        let case_id = CaseId::new("case-123").expect("case id is valid");
        let role = Role::new("developer").expect("role is valid");
        let bundle = RehydrationBundle::new(
            case_id.clone(),
            role.clone(),
            rehydration_domain::BundleNode::new(
                case_id.as_str(),
                "capability",
                "Node case-123",
                "bundle for node case-123 role developer",
                "ACTIVE",
                vec!["projection-node".to_string()],
                BTreeMap::new(),
            ),
            Vec::new(),
            Vec::new(),
            vec![rehydration_domain::BundleNodeDetail::new(
                case_id.as_str(),
                "bundle for node case-123 role developer",
                "pending",
                1,
            )],
            rehydration_domain::BundleMetadata::initial("0.1.0"),
        )
        .expect("bundle should be valid");

        store
            .save_bundle(&bundle)
            .await
            .expect("save should succeed");
    }

    #[test]
    fn mutation_variants_stay_node_centric() {
        let mutation = ProjectionMutation::UpsertNodeDetail(NodeDetailProjection {
            node_id: "node-123".to_string(),
            detail: "expanded detail".to_string(),
            content_hash: "hash-1".to_string(),
            revision: 1,
        });

        match mutation {
            ProjectionMutation::UpsertNodeDetail(detail) => assert_eq!(detail.node_id, "node-123"),
            other => panic!("unexpected mutation: {other:?}"),
        }
    }
}
