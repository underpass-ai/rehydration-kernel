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
OPTIONAL MATCH (source:ProjectionNode)-[edge:RELATED_TO]-(target:ProjectionNode)
WHERE source.node_id = $root_node_id OR target.node_id = $root_node_id
WITH source, edge, target,
     CASE WHEN source.node_id = $root_node_id THEN target ELSE source END AS neighbor
RETURN coalesce(neighbor.node_id, '') AS neighbor_node_id,
       coalesce(neighbor.node_kind, '') AS neighbor_node_kind,
       coalesce(neighbor.title, '') AS neighbor_title,
       coalesce(neighbor.summary, '') AS neighbor_summary,
       coalesce(neighbor.status, '') AS neighbor_status,
       coalesce(neighbor.node_labels, []) AS neighbor_node_labels,
       coalesce(neighbor.properties_json, '{}') AS neighbor_properties_json,
       coalesce(source.node_id, '') AS source_node_id,
       coalesce(target.node_id, '') AS target_node_id,
       coalesce(edge.relation_type, '') AS relation_type
";
