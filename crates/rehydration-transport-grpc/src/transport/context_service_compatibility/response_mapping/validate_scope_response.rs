use rehydration_application::ScopeValidation;
use rehydration_proto::fleet_context_v1::ValidateScopeResponse;

use crate::transport::context_service_compatibility::scope_policy::format_scope_reason;

pub(crate) fn proto_validate_scope_response(result: &ScopeValidation) -> ValidateScopeResponse {
    ValidateScopeResponse {
        allowed: result.allowed,
        missing: result.missing_scopes.clone(),
        extra: result.extra_scopes.clone(),
        reason: format_scope_reason(result),
    }
}

#[cfg(test)]
mod tests {
    use rehydration_application::ScopeValidation;

    use super::proto_validate_scope_response;

    #[test]
    fn validate_scope_response_formats_allowed_reason() {
        let response = proto_validate_scope_response(&ScopeValidation {
            allowed: true,
            required_scopes: vec!["CASE_HEADER".to_string()],
            provided_scopes: vec!["CASE_HEADER".to_string()],
            missing_scopes: Vec::new(),
            extra_scopes: Vec::new(),
            reason: "scope validation passed".to_string(),
            diagnostics: Vec::new(),
        });

        assert!(response.allowed);
        assert_eq!(response.reason, "All scopes are allowed");
    }

    #[test]
    fn validate_scope_response_formats_missing_and_extra_scopes() {
        let response = proto_validate_scope_response(&ScopeValidation {
            allowed: false,
            required_scopes: vec!["CASE_HEADER".to_string()],
            provided_scopes: vec!["graph".to_string()],
            missing_scopes: vec!["CASE_HEADER".to_string()],
            extra_scopes: vec!["graph".to_string()],
            reason: "scope validation failed".to_string(),
            diagnostics: Vec::new(),
        });

        assert!(!response.allowed);
        assert_eq!(response.missing, vec!["CASE_HEADER".to_string()]);
        assert_eq!(response.extra, vec!["graph".to_string()]);
        assert_eq!(
            response.reason,
            "Missing required scopes: CASE_HEADER; Extra scopes not allowed: graph"
        );
    }
}
