use rehydration_application::{
    GetGraphRelationshipsResult, GraphNodeView, GraphRelationshipView, NodeDetailView,
};
use rehydration_domain::{BundleNode, BundleNodeDetail, BundleRelationship};
use rehydration_proto::v1alpha1::{
    BundleNodeDetail as ProtoBundleNodeDetail, GetGraphRelationshipsResponse, GraphNode,
    GraphRelationship,
};

use crate::transport::support::timestamp_from;

pub(crate) fn proto_graph_relationships_response(
    result: &GetGraphRelationshipsResult,
) -> GetGraphRelationshipsResponse {
    GetGraphRelationshipsResponse {
        root: Some(proto_graph_node(&result.root)),
        neighbors: result.neighbors.iter().map(proto_graph_node).collect(),
        relationships: result
            .relationships
            .iter()
            .map(proto_graph_relationship)
            .collect(),
        observed_at: Some(timestamp_from(result.observed_at)),
    }
}

pub(crate) fn proto_graph_node(node: &GraphNodeView) -> GraphNode {
    GraphNode {
        node_id: node.node_id.clone(),
        node_kind: node.node_kind.clone(),
        title: node.title.clone(),
        summary: node.summary.clone(),
        status: node.status.clone(),
        labels: node.labels.clone(),
        properties: node.properties.clone().into_iter().collect(),
    }
}

pub(crate) fn proto_graph_relationship(relationship: &GraphRelationshipView) -> GraphRelationship {
    GraphRelationship {
        source_node_id: relationship.source_node_id.clone(),
        target_node_id: relationship.target_node_id.clone(),
        relationship_type: relationship.relationship_type.clone(),
        properties: relationship
            .explanation
            .to_properties()
            .into_iter()
            .collect(),
    }
}

pub(crate) fn proto_bundle_node(node: &BundleNode) -> GraphNode {
    GraphNode {
        node_id: node.node_id().to_string(),
        node_kind: node.node_kind().to_string(),
        title: node.title().to_string(),
        summary: node.summary().to_string(),
        status: node.status().to_string(),
        labels: node.labels().to_vec(),
        properties: node.properties().clone().into_iter().collect(),
    }
}

pub(crate) fn proto_bundle_relationship(relationship: &BundleRelationship) -> GraphRelationship {
    GraphRelationship {
        source_node_id: relationship.source_node_id().to_string(),
        target_node_id: relationship.target_node_id().to_string(),
        relationship_type: relationship.relationship_type().to_string(),
        properties: relationship
            .explanation()
            .to_properties()
            .into_iter()
            .collect(),
    }
}

pub(crate) fn proto_bundle_node_detail(detail: &BundleNodeDetail) -> ProtoBundleNodeDetail {
    ProtoBundleNodeDetail {
        node_id: detail.node_id().to_string(),
        detail: detail.detail().to_string(),
        content_hash: detail.content_hash().to_string(),
        revision: detail.revision(),
    }
}

pub(crate) fn proto_node_detail_view(detail: &NodeDetailView) -> ProtoBundleNodeDetail {
    ProtoBundleNodeDetail {
        node_id: detail.node_id.clone(),
        detail: detail.detail.clone(),
        content_hash: detail.content_hash.clone(),
        revision: detail.revision,
    }
}
