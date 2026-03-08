use rehydration_application::ScopeValidation;
use rehydration_proto::v1alpha1::ScopeValidationResult;

pub(crate) fn proto_scope_validation(result: &ScopeValidation) -> ScopeValidationResult {
    ScopeValidationResult {
        allowed: result.allowed,
        required_scopes: result.required_scopes.clone(),
        provided_scopes: result.provided_scopes.clone(),
        missing_scopes: result.missing_scopes.clone(),
        extra_scopes: result.extra_scopes.clone(),
        reason: result.reason.clone(),
        diagnostics: result.diagnostics.clone(),
    }
}
