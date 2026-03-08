use neo4rs::{Query, query};

pub(crate) fn load_neighborhood_query(root_node_id: &str) -> Query {
    query(
        "
MATCH (root:ProjectionNode {node_id: $root_node_id})
OPTIONAL MATCH (root)-[edge:RELATED_TO]-(neighbor:ProjectionNode)
RETURN coalesce(neighbor.node_id, '') AS neighbor_node_id,
       coalesce(neighbor.node_kind, '') AS neighbor_node_kind,
       coalesce(neighbor.title, '') AS neighbor_title,
       coalesce(neighbor.summary, '') AS neighbor_summary,
       coalesce(neighbor.status, '') AS neighbor_status,
       coalesce(neighbor.node_labels, []) AS neighbor_node_labels,
       coalesce(neighbor.properties_json, '{}') AS neighbor_properties_json,
       CASE WHEN edge IS NULL THEN '' ELSE startNode(edge).node_id END AS source_node_id,
       CASE WHEN edge IS NULL THEN '' ELSE endNode(edge).node_id END AS target_node_id,
       coalesce(edge.relation_type, '') AS relation_type
        ",
    )
    .param("root_node_id", root_node_id)
}
