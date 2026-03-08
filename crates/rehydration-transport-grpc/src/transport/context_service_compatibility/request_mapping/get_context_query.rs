use rehydration_application::GetContextQuery;
use rehydration_proto::fleet_context_v1::GetContextRequest;

pub(crate) fn map_get_context_query(request: GetContextRequest) -> GetContextQuery {
    GetContextQuery {
        root_node_id: request.story_id,
        role: request.role,
        requested_scopes: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use rehydration_proto::fleet_context_v1::GetContextRequest;

    use super::map_get_context_query;

    #[test]
    fn get_context_query_uses_external_story_id_at_the_boundary() {
        let query = map_get_context_query(GetContextRequest {
            story_id: "story-123".to_string(),
            role: "DEV".to_string(),
            phase: "BUILD".to_string(),
            subtask_id: "task-7".to_string(),
            token_budget: 1024,
        });

        assert_eq!(query.root_node_id, "story-123");
        assert_eq!(query.role, "DEV");
        assert!(query.requested_scopes.is_empty());
    }
}
