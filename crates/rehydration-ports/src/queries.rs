use std::collections::BTreeMap;

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
