use std::collections::BTreeMap;

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ProjectionEnvelope {
    pub event_id: String,
    pub correlation_id: String,
    pub causation_id: String,
    pub occurred_at: String,
    pub aggregate_id: String,
    pub aggregate_type: String,
    pub schema_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct RelatedNodeReference {
    pub node_id: String,
    pub relation_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct GraphNodeMaterializedData {
    pub node_id: String,
    pub node_kind: String,
    pub title: String,
    pub summary: String,
    pub status: String,
    pub labels: Vec<String>,
    pub properties: BTreeMap<String, String>,
    pub related_nodes: Vec<RelatedNodeReference>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct GraphNodeMaterializedEvent {
    #[serde(flatten)]
    pub envelope: ProjectionEnvelope,
    pub data: GraphNodeMaterializedData,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct NodeDetailMaterializedData {
    pub node_id: String,
    pub detail: String,
    pub content_hash: String,
    pub revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct NodeDetailMaterializedEvent {
    #[serde(flatten)]
    pub envelope: ProjectionEnvelope,
    pub data: NodeDetailMaterializedData,
}
