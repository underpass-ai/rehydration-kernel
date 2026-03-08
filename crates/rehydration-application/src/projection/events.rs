use serde::{Deserialize, Serialize};

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
pub struct RelatedNodeReference {
    pub node_id: String,
    pub relation_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphNodeMaterializedData {
    pub node_id: String,
    pub node_kind: String,
    pub title: String,
    pub summary: String,
    pub status: String,
    pub labels: Vec<String>,
    pub properties: std::collections::BTreeMap<String, String>,
    pub related_nodes: Vec<RelatedNodeReference>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphNodeMaterializedEvent {
    #[serde(flatten)]
    pub envelope: ProjectionEnvelope,
    pub data: GraphNodeMaterializedData,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeDetailMaterializedData {
    pub node_id: String,
    pub detail: String,
    pub content_hash: String,
    pub revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeDetailMaterializedEvent {
    #[serde(flatten)]
    pub envelope: ProjectionEnvelope,
    pub data: NodeDetailMaterializedData,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectionEvent {
    GraphNodeMaterialized(GraphNodeMaterializedEvent),
    NodeDetailMaterialized(NodeDetailMaterializedEvent),
}

impl ProjectionEvent {
    pub fn event_id(&self) -> &str {
        match self {
            Self::GraphNodeMaterialized(event) => &event.envelope.event_id,
            Self::NodeDetailMaterialized(event) => &event.envelope.event_id,
        }
    }

    pub fn envelope(&self) -> &ProjectionEnvelope {
        match self {
            Self::GraphNodeMaterialized(event) => &event.envelope,
            Self::NodeDetailMaterialized(event) => &event.envelope,
        }
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
    pub applied_mutations: usize,
    pub checkpoint: Option<rehydration_domain::ProjectionCheckpoint>,
}

pub trait ProjectionEventHandler {
    fn handle_projection_event(
        &self,
        request: ProjectionHandlingRequest,
    ) -> impl std::future::Future<Output = Result<ProjectionHandlingResult, crate::ApplicationError>>
    + Send;
}
