use std::collections::BTreeMap;
use std::future::Future;
use std::sync::Arc;
use std::time::SystemTime;

use rehydration_ports::{
    NodeDetailProjection, NodeProjection, NodeRelationProjection, ProcessedEventStore,
    ProjectionCheckpoint, ProjectionCheckpointStore, ProjectionMutation, ProjectionWriter,
};
use serde::{Deserialize, Serialize};

use crate::{ApplicationError, require_non_empty, trim_to_option};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectionEnvelope {
    pub event_id: String,
    pub correlation_id: String,
    pub causation_id: String,
    pub occurred_at: String,
    pub aggregate_id: String,
    pub aggregate_type: String,
    pub schema_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphNodeMaterializedEvent {
    #[serde(flatten)]
    pub envelope: ProjectionEnvelope,
    pub data: GraphNodeMaterializedData,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphNodeMaterializedData {
    pub node_id: String,
    pub node_kind: String,
    pub title: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default = "default_node_status")]
    pub status: String,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub properties: BTreeMap<String, String>,
    #[serde(default)]
    pub related_nodes: Vec<RelatedNodeReference>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelatedNodeReference {
    pub node_id: String,
    #[serde(default)]
    pub node_kind: String,
    pub relation_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeDetailMaterializedEvent {
    #[serde(flatten)]
    pub envelope: ProjectionEnvelope,
    pub data: NodeDetailMaterializedData,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeDetailMaterializedData {
    pub node_id: String,
    pub detail: String,
    pub content_hash: String,
    pub revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectionEvent {
    GraphNodeMaterialized(GraphNodeMaterializedEvent),
    NodeDetailMaterialized(NodeDetailMaterializedEvent),
}

impl ProjectionEvent {
    pub fn event_id(&self) -> &str {
        self.envelope().event_id.as_str()
    }

    pub fn correlation_id(&self) -> &str {
        self.envelope().correlation_id.as_str()
    }

    pub fn occurred_at(&self) -> &str {
        self.envelope().occurred_at.as_str()
    }

    pub fn envelope(&self) -> &ProjectionEnvelope {
        match self {
            Self::GraphNodeMaterialized(event) => &event.envelope,
            Self::NodeDetailMaterialized(event) => &event.envelope,
        }
    }

    fn validate(&self) -> Result<(), ApplicationError> {
        let envelope = self.envelope();
        require_non_empty(envelope.event_id.clone(), "event_id")?;
        require_non_empty(envelope.correlation_id.clone(), "correlation_id")?;
        require_non_empty(envelope.causation_id.clone(), "causation_id")?;
        require_non_empty(envelope.occurred_at.clone(), "occurred_at")?;
        require_non_empty(envelope.aggregate_id.clone(), "aggregate_id")?;
        require_non_empty(envelope.aggregate_type.clone(), "aggregate_type")?;
        require_non_empty(envelope.schema_version.clone(), "schema_version")?;

        match self {
            Self::GraphNodeMaterialized(event) => {
                require_non_empty(event.data.node_id.clone(), "node_id")?;
                require_non_empty(event.data.node_kind.clone(), "node_kind")?;
                require_non_empty(event.data.title.clone(), "title")?;
                for related_node in &event.data.related_nodes {
                    require_non_empty(related_node.node_id.clone(), "related_node_id")?;
                    require_non_empty(related_node.relation_type.clone(), "relation_type")?;
                }
            }
            Self::NodeDetailMaterialized(event) => {
                require_non_empty(event.data.node_id.clone(), "node_id")?;
                require_non_empty(event.data.detail.clone(), "detail")?;
                require_non_empty(event.data.content_hash.clone(), "content_hash")?;
                if event.data.revision == 0 {
                    return Err(ApplicationError::Validation(
                        "revision must be greater than zero".to_string(),
                    ));
                }
            }
        }

        Ok(())
    }

    fn into_mutations(self) -> Result<Vec<ProjectionMutation>, ApplicationError> {
        self.validate()?;

        Ok(match self {
            Self::GraphNodeMaterialized(event) => {
                let GraphNodeMaterializedData {
                    node_id,
                    node_kind,
                    title,
                    summary,
                    status,
                    labels,
                    properties,
                    related_nodes,
                } = event.data;
                let source_node_id = require_non_empty(node_id, "node_id")?;
                let mut mutations = vec![ProjectionMutation::UpsertNode(NodeProjection {
                    node_id: source_node_id.clone(),
                    node_kind: require_non_empty(node_kind, "node_kind")?,
                    title: require_non_empty(title, "title")?,
                    summary: normalize_optional_string(summary),
                    status: default_status_if_empty(status),
                    labels: labels
                        .into_iter()
                        .filter_map(|label| trim_to_option(&label))
                        .collect(),
                    properties: normalize_properties(properties),
                })];

                mutations.extend(
                    related_nodes
                        .into_iter()
                        .map(|related_node| {
                            Ok(ProjectionMutation::UpsertNodeRelation(
                                NodeRelationProjection {
                                    source_node_id: source_node_id.clone(),
                                    target_node_id: require_non_empty(
                                        related_node.node_id,
                                        "related_node_id",
                                    )?,
                                    relation_type: require_non_empty(
                                        related_node.relation_type,
                                        "relation_type",
                                    )?,
                                },
                            ))
                        })
                        .collect::<Result<Vec<_>, ApplicationError>>()?,
                );

                mutations
            }
            Self::NodeDetailMaterialized(event) => {
                vec![ProjectionMutation::UpsertNodeDetail(NodeDetailProjection {
                    node_id: require_non_empty(event.data.node_id, "node_id")?,
                    detail: require_non_empty(event.data.detail, "detail")?,
                    content_hash: require_non_empty(event.data.content_hash, "content_hash")?,
                    revision: event.data.revision,
                })]
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionHandlingRequest {
    pub consumer_name: String,
    pub stream_name: String,
    pub subject: String,
    pub event: ProjectionEvent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionHandlingResult {
    pub event_id: String,
    pub subject: String,
    pub duplicate: bool,
    pub applied_mutations: u32,
    pub checkpoint: Option<ProjectionCheckpoint>,
}

pub trait ProjectionEventHandler {
    fn handle_projection_event(
        &self,
        request: ProjectionHandlingRequest,
    ) -> impl Future<Output = Result<ProjectionHandlingResult, ApplicationError>> + Send;
}

impl<T> ProjectionEventHandler for Arc<T>
where
    T: ProjectionEventHandler + Send + Sync + ?Sized,
{
    async fn handle_projection_event(
        &self,
        request: ProjectionHandlingRequest,
    ) -> Result<ProjectionHandlingResult, ApplicationError> {
        self.as_ref().handle_projection_event(request).await
    }
}

#[derive(Debug)]
pub struct RoutingProjectionWriter<G, D> {
    graph_writer: Arc<G>,
    detail_writer: Arc<D>,
}

impl<G, D> RoutingProjectionWriter<G, D>
where
    G: ProjectionWriter + Send + Sync,
    D: ProjectionWriter + Send + Sync,
{
    pub fn new(graph_writer: Arc<G>, detail_writer: Arc<D>) -> Self {
        Self {
            graph_writer,
            detail_writer,
        }
    }
}

impl<G, D> ProjectionWriter for RoutingProjectionWriter<G, D>
where
    G: ProjectionWriter + Send + Sync,
    D: ProjectionWriter + Send + Sync,
{
    async fn apply_mutations(
        &self,
        mutations: Vec<ProjectionMutation>,
    ) -> Result<(), rehydration_ports::PortError> {
        let (graph_mutations, detail_mutations): (Vec<_>, Vec<_>) = mutations
            .into_iter()
            .partition(|mutation| !matches!(mutation, ProjectionMutation::UpsertNodeDetail(_)));

        if !graph_mutations.is_empty() {
            self.graph_writer.apply_mutations(graph_mutations).await?;
        }
        if !detail_mutations.is_empty() {
            self.detail_writer.apply_mutations(detail_mutations).await?;
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct ProjectionApplicationService<W, E, C> {
    projection_writer: Arc<W>,
    processed_event_store: Arc<E>,
    checkpoint_store: Arc<C>,
}

impl<W, E, C> ProjectionApplicationService<W, E, C>
where
    W: ProjectionWriter + Send + Sync,
    E: ProcessedEventStore + Send + Sync,
    C: ProjectionCheckpointStore + Send + Sync,
{
    pub fn new(
        projection_writer: Arc<W>,
        processed_event_store: Arc<E>,
        checkpoint_store: Arc<C>,
    ) -> Self {
        Self {
            projection_writer,
            processed_event_store,
            checkpoint_store,
        }
    }

    async fn apply_event(
        &self,
        request: ProjectionHandlingRequest,
    ) -> Result<ProjectionHandlingResult, ApplicationError> {
        let consumer_name = require_non_empty(request.consumer_name, "consumer_name")?;
        let stream_name = require_non_empty(request.stream_name, "stream_name")?;
        let subject = require_non_empty(request.subject, "subject")?;
        request.event.validate()?;

        let event_id = request.event.event_id().to_string();
        if self
            .processed_event_store
            .has_processed(&consumer_name, &event_id)
            .await?
        {
            let checkpoint = self
                .checkpoint_store
                .load_checkpoint(&consumer_name, &stream_name)
                .await?;
            return Ok(ProjectionHandlingResult {
                event_id,
                subject,
                duplicate: true,
                applied_mutations: 0,
                checkpoint,
            });
        }

        let correlation_id = request.event.correlation_id().to_string();
        let occurred_at = request.event.occurred_at().to_string();
        let mutations = request.event.into_mutations()?;
        let applied_mutations = mutations.len() as u32;

        self.projection_writer.apply_mutations(mutations).await?;
        self.processed_event_store
            .record_processed(&consumer_name, &event_id)
            .await?;

        let processed_events = self
            .checkpoint_store
            .load_checkpoint(&consumer_name, &stream_name)
            .await?
            .map_or(1, |checkpoint| checkpoint.processed_events + 1);
        let checkpoint = ProjectionCheckpoint {
            consumer_name: consumer_name.clone(),
            stream_name: stream_name.clone(),
            last_subject: subject.clone(),
            last_event_id: event_id.clone(),
            last_correlation_id: correlation_id,
            last_occurred_at: occurred_at,
            processed_events,
            updated_at: SystemTime::now(),
        };
        self.checkpoint_store
            .save_checkpoint(checkpoint.clone())
            .await?;

        Ok(ProjectionHandlingResult {
            event_id,
            subject,
            duplicate: false,
            applied_mutations,
            checkpoint: Some(checkpoint),
        })
    }
}

impl<W, E, C> ProjectionEventHandler for ProjectionApplicationService<W, E, C>
where
    W: ProjectionWriter + Send + Sync,
    E: ProcessedEventStore + Send + Sync,
    C: ProjectionCheckpointStore + Send + Sync,
{
    async fn handle_projection_event(
        &self,
        request: ProjectionHandlingRequest,
    ) -> Result<ProjectionHandlingResult, ApplicationError> {
        self.apply_event(request).await
    }
}

fn normalize_optional_string(value: String) -> String {
    trim_to_option(&value).unwrap_or_default()
}

fn default_node_status() -> String {
    "STATUS_UNSPECIFIED".to_string()
}

fn default_status_if_empty(value: String) -> String {
    trim_to_option(&value).unwrap_or_else(default_node_status)
}

fn normalize_properties(properties: BTreeMap<String, String>) -> BTreeMap<String, String> {
    properties
        .into_iter()
        .filter_map(|(key, value)| {
            let key = trim_to_option(&key)?;
            let value = trim_to_option(&value)?;
            Some((key, value))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use rehydration_ports::{ProjectionMutation, ProjectionWriter};
    use rehydration_testkit::{
        InMemoryProcessedEventStore, InMemoryProjectionCheckpointStore, InMemoryProjectionWriter,
    };

    use super::{
        GraphNodeMaterializedData, GraphNodeMaterializedEvent, NodeDetailMaterializedData,
        NodeDetailMaterializedEvent, ProjectionApplicationService, ProjectionEnvelope,
        ProjectionEvent, ProjectionEventHandler, ProjectionHandlingRequest, RelatedNodeReference,
        RoutingProjectionWriter,
    };

    fn envelope(event_id: &str, aggregate_id: &str) -> ProjectionEnvelope {
        ProjectionEnvelope {
            event_id: event_id.to_string(),
            correlation_id: format!("corr-{event_id}"),
            causation_id: format!("cause-{event_id}"),
            occurred_at: "2026-03-07T19:00:00Z".to_string(),
            aggregate_id: aggregate_id.to_string(),
            aggregate_type: "node".to_string(),
            schema_version: "v1alpha1".to_string(),
        }
    }

    #[tokio::test]
    async fn graph_node_event_creates_node_and_relations_with_checkpoint() {
        let writer = Arc::new(InMemoryProjectionWriter::default());
        let processed = Arc::new(InMemoryProcessedEventStore::default());
        let checkpoints = Arc::new(InMemoryProjectionCheckpointStore::default());
        let service = ProjectionApplicationService::new(
            Arc::clone(&writer),
            Arc::clone(&processed),
            Arc::clone(&checkpoints),
        );

        let result = service
            .handle_projection_event(ProjectionHandlingRequest {
                consumer_name: "context-projection".to_string(),
                stream_name: "rehydration.events".to_string(),
                subject: "graph.node.materialized".to_string(),
                event: ProjectionEvent::GraphNodeMaterialized(GraphNodeMaterializedEvent {
                    envelope: envelope("evt-1", "node-123"),
                    data: GraphNodeMaterializedData {
                        node_id: "node-123".to_string(),
                        node_kind: "capability".to_string(),
                        title: "Projection consumer foundation".to_string(),
                        summary: "Node centric projection input".to_string(),
                        status: "ACTIVE".to_string(),
                        labels: vec!["projection".to_string(), "foundation".to_string()],
                        properties: BTreeMap::from([
                            ("role".to_string(), "developer".to_string()),
                            ("phase".to_string(), "build".to_string()),
                        ]),
                        related_nodes: vec![
                            RelatedNodeReference {
                                node_id: "node-122".to_string(),
                                node_kind: "decision".to_string(),
                                relation_type: "depends_on".to_string(),
                            },
                            RelatedNodeReference {
                                node_id: "node-121".to_string(),
                                node_kind: "milestone".to_string(),
                                relation_type: "blocked_by".to_string(),
                            },
                        ],
                    },
                }),
            })
            .await
            .expect("graph node event should be applied");

        assert!(!result.duplicate);
        assert_eq!(result.applied_mutations, 3);

        let mutations = writer.mutations().await;
        assert_eq!(mutations.len(), 3);
        assert!(matches!(mutations[0], ProjectionMutation::UpsertNode(_)));
        assert!(matches!(
            mutations[1],
            ProjectionMutation::UpsertNodeRelation(_)
        ));
        assert!(matches!(
            mutations[2],
            ProjectionMutation::UpsertNodeRelation(_)
        ));

        let checkpoint = checkpoints
            .checkpoint("context-projection", "rehydration.events")
            .await
            .expect("checkpoint should exist");
        assert_eq!(checkpoint.last_event_id, "evt-1");
        assert_eq!(checkpoint.processed_events, 1);
        assert_eq!(processed.processed().await.len(), 1);
    }

    #[tokio::test]
    async fn duplicate_event_is_deduplicated_without_new_mutations() {
        let writer = Arc::new(InMemoryProjectionWriter::default());
        let processed = Arc::new(InMemoryProcessedEventStore::default());
        let checkpoints = Arc::new(InMemoryProjectionCheckpointStore::default());
        let service = ProjectionApplicationService::new(
            Arc::clone(&writer),
            Arc::clone(&processed),
            Arc::clone(&checkpoints),
        );

        let request = ProjectionHandlingRequest {
            consumer_name: "context-projection".to_string(),
            stream_name: "rehydration.events".to_string(),
            subject: "graph.node.materialized".to_string(),
            event: ProjectionEvent::GraphNodeMaterialized(GraphNodeMaterializedEvent {
                envelope: envelope("evt-2", "node-123"),
                data: GraphNodeMaterializedData {
                    node_id: "node-123".to_string(),
                    node_kind: "capability".to_string(),
                    title: "Projection consumer foundation".to_string(),
                    summary: String::new(),
                    status: "ACTIVE".to_string(),
                    labels: Vec::new(),
                    properties: BTreeMap::new(),
                    related_nodes: Vec::new(),
                },
            }),
        };

        service
            .handle_projection_event(request.clone())
            .await
            .expect("first event should be applied");
        let duplicate = service
            .handle_projection_event(request)
            .await
            .expect("duplicate event should return success");

        assert!(duplicate.duplicate);
        assert_eq!(duplicate.applied_mutations, 0);
        assert_eq!(writer.mutations().await.len(), 1);
        let checkpoint = duplicate.checkpoint.expect("checkpoint should be returned");
        assert_eq!(checkpoint.processed_events, 1);
    }

    #[tokio::test]
    async fn node_detail_event_maps_to_valkey_detail_mutation() {
        let writer = Arc::new(InMemoryProjectionWriter::default());
        let service = ProjectionApplicationService::new(
            Arc::clone(&writer),
            Arc::new(InMemoryProcessedEventStore::default()),
            Arc::new(InMemoryProjectionCheckpointStore::default()),
        );

        service
            .handle_projection_event(ProjectionHandlingRequest {
                consumer_name: "context-projection".to_string(),
                stream_name: "rehydration.events".to_string(),
                subject: "node.detail.materialized".to_string(),
                event: ProjectionEvent::NodeDetailMaterialized(NodeDetailMaterializedEvent {
                    envelope: envelope("evt-3", "node-123"),
                    data: NodeDetailMaterializedData {
                        node_id: "node-123".to_string(),
                        detail: "Expanded node detail from source system".to_string(),
                        content_hash: "hash-123".to_string(),
                        revision: 3,
                    },
                }),
            })
            .await
            .expect("detail event should be applied");

        let mutations = writer.mutations().await;
        match &mutations[0] {
            ProjectionMutation::UpsertNodeDetail(detail) => {
                assert_eq!(detail.node_id, "node-123");
                assert_eq!(detail.revision, 3);
            }
            mutation => panic!("unexpected mutation: {mutation:?}"),
        }
    }

    #[tokio::test]
    async fn routing_projection_writer_splits_graph_and_detail_mutations() {
        let graph_writer = Arc::new(InMemoryProjectionWriter::default());
        let detail_writer = Arc::new(InMemoryProjectionWriter::default());
        let writer =
            RoutingProjectionWriter::new(Arc::clone(&graph_writer), Arc::clone(&detail_writer));

        writer
            .apply_mutations(vec![
                ProjectionMutation::UpsertNode(rehydration_ports::NodeProjection {
                    node_id: "node-123".to_string(),
                    node_kind: "capability".to_string(),
                    title: "Projection foundation".to_string(),
                    summary: String::new(),
                    status: "ACTIVE".to_string(),
                    labels: Vec::new(),
                    properties: BTreeMap::new(),
                }),
                ProjectionMutation::UpsertNodeDetail(rehydration_ports::NodeDetailProjection {
                    node_id: "node-123".to_string(),
                    detail: "Expanded detail".to_string(),
                    content_hash: "hash-123".to_string(),
                    revision: 1,
                }),
            ])
            .await
            .expect("mutations should be routed");

        assert_eq!(graph_writer.mutations().await.len(), 1);
        assert_eq!(detail_writer.mutations().await.len(), 1);
    }
}
