use rehydration_application::{ContextRenderOptions, GetContextQuery};
use rehydration_proto::fleet_context_v1::GetContextRequest;

use crate::transport::context_service_compatibility::scope_policy::expected_scopes;
use crate::transport::support::trim_to_option;

pub(crate) fn map_get_context_query(request: GetContextRequest) -> GetContextQuery {
    GetContextQuery {
        requested_scopes: expected_scopes(&request.phase, &request.role),
        render_options: ContextRenderOptions {
            focus_node_id: trim_to_option(request.subtask_id),
            token_budget: positive_token_budget(request.token_budget),
        },
        root_node_id: request.story_id,
        role: request.role,
    }
}

fn positive_token_budget(value: i32) -> Option<u32> {
    (value > 0).then_some(value as u32)
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
        assert_eq!(
            query.render_options.focus_node_id.as_deref(),
            Some("task-7")
        );
        assert_eq!(query.render_options.token_budget, Some(1024));
        assert!(query.requested_scopes.is_empty());
    }

    #[test]
    fn get_context_query_uses_phase_to_derive_expected_scopes() {
        let query = map_get_context_query(GetContextRequest {
            story_id: "story-123".to_string(),
            role: "developer".to_string(),
            phase: "BUILD".to_string(),
            subtask_id: String::new(),
            token_budget: 0,
        });

        assert_eq!(
            query.requested_scopes,
            vec![
                "CASE_HEADER".to_string(),
                "PLAN_HEADER".to_string(),
                "SUBTASKS_ROLE".to_string(),
                "DECISIONS_RELEVANT_ROLE".to_string(),
                "DEPS_RELEVANT".to_string(),
            ]
        );
        assert_eq!(query.render_options.focus_node_id, None);
        assert_eq!(query.render_options.token_budget, None);
    }
}
