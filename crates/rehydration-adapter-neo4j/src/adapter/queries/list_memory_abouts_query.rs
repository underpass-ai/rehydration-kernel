use neo4rs::Query;

pub(crate) fn list_memory_abouts_query() -> Query {
    Query::new(
        "
MATCH (anchor:ProjectionNode)
WHERE anchor.node_kind = 'memory_anchor'
RETURN anchor.node_id AS about
ORDER BY about
        "
        .to_string(),
    )
}
