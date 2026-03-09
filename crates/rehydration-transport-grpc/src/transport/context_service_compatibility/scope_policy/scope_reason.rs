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

#[cfg(test)]
mod tests {
    use rehydration_application::ScopeValidation;

    use super::format_scope_reason;

    #[test]
    fn format_scope_reason_matches_allowed_contract() {
        let reason = format_scope_reason(&ScopeValidation {
            allowed: true,
            required_scopes: Vec::new(),
            provided_scopes: Vec::new(),
            missing_scopes: Vec::new(),
            extra_scopes: Vec::new(),
            reason: String::new(),
            diagnostics: Vec::new(),
        });

        assert_eq!(reason, "All scopes are allowed");
    }

    #[test]
    fn format_scope_reason_matches_missing_and_extra_contract() {
        let reason = format_scope_reason(&ScopeValidation {
            allowed: false,
            required_scopes: Vec::new(),
            provided_scopes: Vec::new(),
            missing_scopes: vec!["admin".to_string(), "write".to_string()],
            extra_scopes: vec!["invalid".to_string()],
            reason: String::new(),
            diagnostics: Vec::new(),
        });

        assert_eq!(
            reason,
            "Missing required scopes: admin, write; Extra scopes not allowed: invalid"
        );
    }
}
