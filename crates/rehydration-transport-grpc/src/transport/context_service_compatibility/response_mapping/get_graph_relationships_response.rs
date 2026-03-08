use rehydration_application::{GetGraphRelationshipsResult, GraphNodeView, GraphRelationshipView};
use rehydration_proto::fleet_context_v1::{
    GetGraphRelationshipsResponse, GraphNode, GraphRelationship,
};

pub(crate) fn proto_get_graph_relationships_response(
    result: &GetGraphRelationshipsResult,
) -> GetGraphRelationshipsResponse {
    GetGraphRelationshipsResponse {
        node: Some(proto_graph_node(&result.root)),
        neighbors: result.neighbors.iter().map(proto_graph_node).collect(),
        relationships: result
            .relationships
            .iter()
            .map(proto_graph_relationship)
            .collect(),
        success: true,
        message: String::new(),
    }
}

fn proto_graph_node(node: &GraphNodeView) -> GraphNode {
    let mut properties = node.properties.clone();
    if !node.summary.is_empty() {
        properties.insert("summary".to_string(), node.summary.clone());
    }
    if !node.status.is_empty() {
        properties.insert("status".to_string(), node.status.clone());
    }

    GraphNode {
        id: node.node_id.clone(),
        labels: node.labels.clone(),
        properties: properties.into_iter().collect(),
        r#type: node.node_kind.clone(),
        title: node.title.clone(),
    }
}

fn proto_graph_relationship(relationship: &GraphRelationshipView) -> GraphRelationship {
    GraphRelationship {
        from_node_id: relationship.source_node_id.clone(),
        to_node_id: relationship.target_node_id.clone(),
        r#type: relationship.relationship_type.clone(),
        properties: relationship.properties.clone().into_iter().collect(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use rehydration_application::{
        GetGraphRelationshipsResult, GraphNodeView, GraphRelationshipView,
    };

    use super::proto_get_graph_relationships_response;

    #[test]
    fn graph_relationship_response_folds_summary_and_status_into_properties() {
        let response = proto_get_graph_relationships_response(&GetGraphRelationshipsResult {
            root: GraphNodeView {
                node_id: "node-1".to_string(),
                node_kind: "Story".to_string(),
                title: "Story".to_string(),
                summary: "Summary".to_string(),
                status: "ACTIVE".to_string(),
                labels: vec!["Story".to_string()],
                properties: BTreeMap::new(),
            },
            neighbors: Vec::new(),
            relationships: vec![GraphRelationshipView {
                source_node_id: "node-1".to_string(),
                target_node_id: "node-2".to_string(),
                relationship_type: "DEPENDS_ON".to_string(),
                properties: BTreeMap::new(),
            }],
            observed_at: std::time::SystemTime::now(),
        });

        let node = response.node.expect("node");
        assert_eq!(
            node.properties.get("summary").map(String::as_str),
            Some("Summary")
        );
        assert_eq!(
            node.properties.get("status").map(String::as_str),
            Some("ACTIVE")
        );
        assert_eq!(response.relationships[0].r#type, "DEPENDS_ON");
    }
}
