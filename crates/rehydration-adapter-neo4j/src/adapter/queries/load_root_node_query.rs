use neo4rs::{Query, query};

pub(crate) fn load_root_node_query(root_node_id: &str) -> Query {
    query(
        "
MATCH (root:ProjectionNode {node_id: $root_node_id})
RETURN root.node_id AS node_id,
       coalesce(root.node_kind, '') AS node_kind,
       coalesce(root.title, '') AS title,
       coalesce(root.summary, '') AS summary,
       coalesce(root.status, '') AS status,
       coalesce(root.node_labels, []) AS node_labels,
       coalesce(root.properties_json, '{}') AS properties_json
LIMIT 1
        ",
    )
    .param("root_node_id", root_node_id)
}
