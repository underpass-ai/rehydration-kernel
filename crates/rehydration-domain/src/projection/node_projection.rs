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
