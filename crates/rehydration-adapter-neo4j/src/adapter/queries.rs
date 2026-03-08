pub(crate) const ROOT_NODE_QUERY: &str = "
MATCH (root:ProjectionNode {node_id: $root_node_id})
RETURN root.node_id AS node_id,
       coalesce(root.node_kind, '') AS node_kind,
       coalesce(root.title, '') AS title,
       coalesce(root.summary, '') AS summary,
       coalesce(root.status, '') AS status,
       coalesce(root.node_labels, []) AS node_labels,
       coalesce(root.properties_json, '{}') AS properties_json
LIMIT 1
";

pub(crate) const NODE_NEIGHBORHOOD_QUERY: &str = "
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
";
