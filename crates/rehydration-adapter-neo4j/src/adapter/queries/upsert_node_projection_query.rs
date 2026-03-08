use neo4rs::{Query, query};
use rehydration_ports::{NodeProjection, PortError};

use super::super::row_mapping::serialize_properties;

pub(crate) fn upsert_node_projection_query(node: &NodeProjection) -> Result<Query, PortError> {
    Ok(query(
        "
MERGE (node:ProjectionNode {node_id: $node_id})
SET node.node_kind = $node_kind,
    node.title = $title,
    node.summary = $summary,
    node.status = $status,
    node.node_labels = $node_labels,
    node.properties_json = $properties_json
        ",
    )
    .param("node_id", node.node_id.as_str())
    .param("node_kind", node.node_kind.as_str())
    .param("title", node.title.as_str())
    .param("summary", node.summary.as_str())
    .param("status", node.status.as_str())
    .param("node_labels", node.labels.clone())
    .param("properties_json", serialize_properties(&node.properties)?))
}
