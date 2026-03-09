use rehydration_application::ScopeValidation;

pub(crate) fn format_scope_reason(result: &ScopeValidation) -> String {
    if result.allowed {
        return "All scopes are allowed".to_string();
    }

    let mut parts = Vec::new();

    if !result.missing_scopes.is_empty() {
        parts.push(format!(
            "Missing required scopes: {}",
            result.missing_scopes.join(", ")
        ));
    }

    if !result.extra_scopes.is_empty() {
        parts.push(format!(
            "Extra scopes not allowed: {}",
            result.extra_scopes.join(", ")
        ));
    }

    parts.join("; ")
}
