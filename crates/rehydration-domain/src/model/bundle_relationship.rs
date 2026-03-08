use std::collections::BTreeMap;

use crate::NodeRelationProjection;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleRelationship {
    source_node_id: String,
    target_node_id: String,
    relationship_type: String,
    properties: BTreeMap<String, String>,
}

impl BundleRelationship {
    pub fn new(
        source_node_id: impl Into<String>,
        target_node_id: impl Into<String>,
        relationship_type: impl Into<String>,
        properties: BTreeMap<String, String>,
    ) -> Self {
        Self {
            source_node_id: source_node_id.into(),
            target_node_id: target_node_id.into(),
            relationship_type: relationship_type.into(),
            properties,
        }
    }

    pub fn from_projection(relationship: &NodeRelationProjection) -> Self {
        Self {
            source_node_id: relationship.source_node_id.clone(),
            target_node_id: relationship.target_node_id.clone(),
            relationship_type: relationship.relation_type.clone(),
            properties: BTreeMap::new(),
        }
    }

    pub fn source_node_id(&self) -> &str {
        &self.source_node_id
    }

    pub fn target_node_id(&self) -> &str {
        &self.target_node_id
    }

    pub fn relationship_type(&self) -> &str {
        &self.relationship_type
    }

    pub fn properties(&self) -> &BTreeMap<String, String> {
        &self.properties
    }
}
