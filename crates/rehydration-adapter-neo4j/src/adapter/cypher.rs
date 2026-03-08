use neo4rs::{Query, query};

pub(crate) fn node_scoped_query(statement: &str, root_node_id: &str) -> Query {
    query(statement).param("root_node_id", root_node_id)
}
