use neo4rs::{Query, query};
use rehydration_domain::{CaseId, Role};

pub(crate) fn scoped_query(statement: &str, case_id: &CaseId, role: &Role) -> Query {
    query(statement)
        .param("case_id", case_id.as_str())
        .param("role", role.as_str())
}

pub(crate) fn node_scoped_query(statement: &str, root_node_id: &str) -> Query {
    query(statement).param("root_node_id", root_node_id)
}
