use rehydration_application::ValidateScopeQuery;
use rehydration_proto::fleet_context_v1::ValidateScopeRequest;

use crate::transport::context_service_compatibility::scope_policy::expected_scopes;

pub(crate) fn map_validate_scope_query(request: ValidateScopeRequest) -> ValidateScopeQuery {
    ValidateScopeQuery {
        required_scopes: expected_scopes(&request.phase, &request.role),
        provided_scopes: request.provided_scopes,
    }
}

#[cfg(test)]
mod tests {
    use rehydration_proto::fleet_context_v1::ValidateScopeRequest;

    use super::map_validate_scope_query;

    #[test]
    fn validate_scope_query_uses_compatibility_scope_catalog() {
        let query = map_validate_scope_query(ValidateScopeRequest {
            role: "developer".to_string(),
            phase: "BUILD".to_string(),
            provided_scopes: vec!["SUBTASKS_ROLE".to_string()],
        });

        assert_eq!(
            query.required_scopes,
            vec![
                "CASE_HEADER".to_string(),
                "PLAN_HEADER".to_string(),
                "SUBTASKS_ROLE".to_string(),
                "DECISIONS_RELEVANT_ROLE".to_string(),
                "DEPS_RELEVANT".to_string(),
            ]
        );
        assert_eq!(query.provided_scopes, vec!["SUBTASKS_ROLE".to_string()]);
    }

    #[test]
    fn validate_scope_query_returns_empty_requirements_for_unknown_role_or_phase() {
        let query = map_validate_scope_query(ValidateScopeRequest {
            role: "DEV".to_string(),
            phase: "BUILD".to_string(),
            provided_scopes: vec!["graph".to_string()],
        });

        assert!(query.required_scopes.is_empty());
        assert_eq!(query.provided_scopes, vec!["graph".to_string()]);
    }
}
