use std::collections::BTreeSet;

use crate::{
    BundleMetadata, BundleNode, BundleNodeDetail, BundleRelationship, CaseId, DomainError,
    RehydrationStats, Role,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RehydrationBundle {
    root_node_id: CaseId,
    role: Role,
    root_node: BundleNode,
    neighbor_nodes: Vec<BundleNode>,
    relationships: Vec<BundleRelationship>,
    node_details: Vec<BundleNodeDetail>,
    stats: RehydrationStats,
    metadata: BundleMetadata,
}

impl RehydrationBundle {
    pub fn new(
        root_node_id: CaseId,
        role: Role,
        root_node: BundleNode,
        neighbor_nodes: Vec<BundleNode>,
        relationships: Vec<BundleRelationship>,
        node_details: Vec<BundleNodeDetail>,
        metadata: BundleMetadata,
    ) -> Result<Self, DomainError> {
        if root_node.node_id() != root_node_id.as_str() {
            return Err(DomainError::InvalidState(format!(
                "root node id `{}` does not match bundle root `{}`",
                root_node.node_id(),
                root_node_id.as_str()
            )));
        }

        let mut node_ids = BTreeSet::new();
        node_ids.insert(root_node.node_id().to_string());
        for node in &neighbor_nodes {
            if !node_ids.insert(node.node_id().to_string()) {
                return Err(DomainError::InvalidState(format!(
                    "duplicate bundle node `{}`",
                    node.node_id()
                )));
            }
        }

        for relationship in &relationships {
            if !node_ids.contains(relationship.source_node_id())
                || !node_ids.contains(relationship.target_node_id())
            {
                return Err(DomainError::InvalidState(format!(
                    "relationship `{}` -> `{}` references nodes outside the bundle",
                    relationship.source_node_id(),
                    relationship.target_node_id()
                )));
            }
        }

        for detail in &node_details {
            if !node_ids.contains(detail.node_id()) {
                return Err(DomainError::InvalidState(format!(
                    "node detail `{}` does not belong to this bundle",
                    detail.node_id()
                )));
            }
        }

        let stats = RehydrationStats::new(
            node_ids.len() as u32,
            relationships.len() as u32,
            node_details.len() as u32,
        );

        Ok(Self {
            root_node_id,
            role,
            root_node,
            neighbor_nodes,
            relationships,
            node_details,
            stats,
            metadata,
        })
    }

    pub fn root_node_id(&self) -> &CaseId {
        &self.root_node_id
    }

    pub fn role(&self) -> &Role {
        &self.role
    }

    pub fn root_node(&self) -> &BundleNode {
        &self.root_node
    }

    pub fn neighbor_nodes(&self) -> &[BundleNode] {
        &self.neighbor_nodes
    }

    pub fn relationships(&self) -> &[BundleRelationship] {
        &self.relationships
    }

    pub fn node_details(&self) -> &[BundleNodeDetail] {
        &self.node_details
    }

    pub fn stats(&self) -> &RehydrationStats {
        &self.stats
    }

    pub fn metadata(&self) -> &BundleMetadata {
        &self.metadata
    }
}
