use neo4rs::{Query, query};

pub(crate) fn load_neighborhood_query(root_node_id: &str) -> Query {
    query(
        "
MATCH (root:ProjectionNode {node_id: $root_node_id})
OPTIONAL MATCH (root)-[:RELATED_TO]-(seed_neighbor:ProjectionNode)
WITH root, [node IN collect(DISTINCT seed_neighbor) WHERE node IS NOT NULL] AS seed_neighbors
UNWIND CASE WHEN size(seed_neighbors) = 0 THEN [NULL] ELSE seed_neighbors END AS neighbor
OPTIONAL MATCH (source:ProjectionNode)-[edge:RELATED_TO]-(target:ProjectionNode)
WHERE neighbor IS NOT NULL
  AND (source = neighbor OR target = neighbor)
  AND (source = root OR source IN seed_neighbors)
  AND (target = root OR target IN seed_neighbors)
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
