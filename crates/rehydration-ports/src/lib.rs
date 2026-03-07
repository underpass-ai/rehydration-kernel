use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::future::Future;
use std::sync::Arc;
use std::time::SystemTime;

use rehydration_domain::{CaseId, RehydrationBundle, Role, RoleContextPack};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortError {
    InvalidState(String),
    Unavailable(String),
}

impl fmt::Display for PortError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidState(message) | Self::Unavailable(message) => f.write_str(message),
        }
    }
}

impl Error for PortError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeProjection {
    pub node_id: String,
    pub node_kind: String,
    pub title: String,
    pub summary: String,
    pub status: String,
    pub labels: Vec<String>,
    pub properties: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeRelationProjection {
    pub source_node_id: String,
    pub target_node_id: String,
    pub relation_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeDetailProjection {
    pub node_id: String,
    pub detail: String,
    pub content_hash: String,
    pub revision: u64,
}

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

pub trait ProjectionReader {
    fn load_pack(
        &self,
        case_id: &CaseId,
        role: &Role,
    ) -> impl Future<Output = Result<Option<RoleContextPack>, PortError>> + Send;
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

impl<T> ProjectionReader for Arc<T>
where
    T: ProjectionReader + Send + Sync + ?Sized,
{
    async fn load_pack(
        &self,
        case_id: &CaseId,
        role: &Role,
    ) -> Result<Option<RoleContextPack>, PortError> {
        self.as_ref().load_pack(case_id, role).await
    }
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

    use rehydration_domain::{CaseHeader, CaseId, RehydrationBundle, Role, RoleContextPack};

    use super::{
        NodeProjection, PortError, ProcessedEventStore, ProjectionCheckpoint,
        ProjectionCheckpointStore, ProjectionMutation, ProjectionReader, ProjectionWriter,
        SnapshotStore,
    };

    struct Reader;

    impl ProjectionReader for Reader {
        async fn load_pack(
            &self,
            case_id: &CaseId,
            role: &Role,
        ) -> Result<Option<RoleContextPack>, PortError> {
            Ok(Some(RoleContextPack::new(
                role.clone(),
                CaseHeader::new(
                    case_id.clone(),
                    "Case 123",
                    "A seeded pack",
                    "ACTIVE",
                    std::time::SystemTime::UNIX_EPOCH,
                    "testkit",
                ),
                None,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                "A seeded pack",
                4096,
            )))
        }
    }

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
                last_event_id: "evt-0".to_string(),
                last_correlation_id: "corr-0".to_string(),
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

    struct Store;

    impl SnapshotStore for Store {
        async fn save_bundle(&self, _bundle: &RehydrationBundle) -> Result<(), PortError> {
            Ok(())
        }
    }

    #[test]
    fn port_error_uses_inner_message() {
        let error = PortError::Unavailable("neo4j unavailable".to_string());
        assert_eq!(error.to_string(), "neo4j unavailable");
    }

    #[tokio::test]
    async fn arc_projection_reader_delegates() {
        let reader = Arc::new(Reader);
        let bundle = reader
            .load_pack(
                &CaseId::new("case-123").expect("case id is valid"),
                &Role::new("developer").expect("role is valid"),
            )
            .await
            .expect("load should succeed");

        assert!(bundle.is_some());
    }

    #[tokio::test]
    async fn arc_projection_writer_delegates() {
        let writer = Arc::new(Writer);

        writer
            .apply_mutations(vec![ProjectionMutation::UpsertNode(NodeProjection {
                node_id: "node-123".to_string(),
                node_kind: "task".to_string(),
                title: "Task 123".to_string(),
                summary: "Projection updated".to_string(),
                status: "ACTIVE".to_string(),
                labels: vec!["work-item".to_string()],
                properties: BTreeMap::from([("role".to_string(), "developer".to_string())]),
            })])
            .await
            .expect("write via arc should succeed");
    }

    #[tokio::test]
    async fn arc_processed_event_store_delegates() {
        let store = Arc::new(EventStore);

        assert!(
            store
                .has_processed("context-projection", "evt-1")
                .await
                .expect("lookup should succeed")
        );
        store
            .record_processed("context-projection", "evt-2")
            .await
            .expect("record should succeed");
    }

    #[tokio::test]
    async fn arc_checkpoint_store_delegates() {
        let store = Arc::new(CheckpointStore);
        let checkpoint = store
            .load_checkpoint("context-projection", "projection.events")
            .await
            .expect("load should succeed")
            .expect("checkpoint should exist");

        assert_eq!(checkpoint.last_event_id, "evt-0");

        store
            .save_checkpoint(checkpoint)
            .await
            .expect("save should succeed");
    }

    #[tokio::test]
    async fn arc_snapshot_store_delegates() {
        let store = Arc::new(Store);
        let bundle = RehydrationBundle::empty(
            CaseId::new("case-123").expect("case id is valid"),
            Role::new("developer").expect("role is valid"),
            "0.1.0",
        );

        store
            .save_bundle(&bundle)
            .await
            .expect("save via arc should succeed");
    }
}
