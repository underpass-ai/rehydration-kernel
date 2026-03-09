use rehydration_application::ApplicationError;

const ALLOWED_NODE_TYPES: &[&str] = &["Project", "Epic", "Story", "Task"];

pub(crate) fn validate_get_graph_relationships_node_type(
    node_type: String,
) -> Result<String, ApplicationError> {
    let node_type = node_type.trim().to_string();

    if ALLOWED_NODE_TYPES.contains(&node_type.as_str()) {
        Ok(node_type)
    } else {
        Err(ApplicationError::Validation(format!(
            "Invalid node_type: {node_type}. Must be Project, Epic, Story, or Task"
        )))
    }
}

#[cfg(test)]
mod tests {
    use rehydration_application::ApplicationError;

    use super::validate_get_graph_relationships_node_type;

    #[test]
    fn node_type_validator_accepts_frozen_external_values() {
        for node_type in ["Project", "Epic", "Story", "Task"] {
            assert_eq!(
                validate_get_graph_relationships_node_type(node_type.to_string())
                    .expect("node type should be accepted"),
                node_type
            );
        }
    }

    #[test]
    fn node_type_validator_rejects_invalid_values() {
        let error = validate_get_graph_relationships_node_type("InvalidType".to_string())
            .expect_err("invalid node type should be rejected");

        assert!(matches!(
            error,
            ApplicationError::Validation(message)
                if message == "Invalid node_type: InvalidType. Must be Project, Epic, Story, or Task"
        ));
    }
}
