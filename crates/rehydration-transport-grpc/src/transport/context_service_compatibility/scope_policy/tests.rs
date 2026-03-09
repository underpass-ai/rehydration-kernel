use rehydration_application::ScopeValidation;

use super::{expected_scopes, format_scope_reason};

#[test]
fn expected_scopes_cover_all_known_phase_role_pairs() {
    let cases = [
        (
            "DESIGN",
            "architect",
            &[
                "CASE_HEADER",
                "PLAN_HEADER",
                "DECISIONS_GLOBAL",
                "DEPS_DECISIONS",
                "MILESTONES",
                "SUMMARY_LAST",
            ][..],
        ),
        (
            "DESIGN",
            "developer",
            &[
                "CASE_HEADER",
                "PLAN_HEADER",
                "DECISIONS_RELEVANT_ROLE",
                "DEPS_RELEVANT",
                "SUMMARY_LAST",
            ][..],
        ),
        (
            "DESIGN",
            "devops",
            &[
                "CASE_HEADER",
                "PLAN_HEADER",
                "DECISIONS_RELEVANT_ROLE",
                "DEPS_RELEVANT",
                "MILESTONES",
            ][..],
        ),
        (
            "DESIGN",
            "qa",
            &[
                "CASE_HEADER",
                "PLAN_HEADER",
                "DECISIONS_GLOBAL",
                "SUBTASKS_ALL",
                "MILESTONES",
                "SUMMARY_LAST",
            ][..],
        ),
        (
            "DESIGN",
            "data",
            &[
                "CASE_HEADER",
                "PLAN_HEADER",
                "DECISIONS_RELEVANT_ROLE",
                "DEPS_RELEVANT",
            ][..],
        ),
        (
            "BUILD",
            "architect",
            &[
                "CASE_HEADER",
                "PLAN_HEADER",
                "DECISIONS_GLOBAL",
                "DEPS_DECISIONS",
                "SUBTASKS_ALL",
                "MILESTONES",
            ][..],
        ),
        (
            "BUILD",
            "developer",
            &[
                "CASE_HEADER",
                "PLAN_HEADER",
                "SUBTASKS_ROLE",
                "DECISIONS_RELEVANT_ROLE",
                "DEPS_RELEVANT",
            ][..],
        ),
        (
            "BUILD",
            "devops",
            &[
                "CASE_HEADER",
                "PLAN_HEADER",
                "SUBTASKS_ROLE",
                "DECISIONS_RELEVANT_ROLE",
                "DEPS_RELEVANT",
            ][..],
        ),
        (
            "BUILD",
            "qa",
            &[
                "CASE_HEADER",
                "PLAN_HEADER",
                "SUBTASKS_ALL_MIN",
                "DECISIONS_GLOBAL_MIN",
                "MILESTONES",
            ][..],
        ),
        (
            "BUILD",
            "data",
            &[
                "CASE_HEADER",
                "PLAN_HEADER",
                "SUBTASKS_ROLE",
                "DECISIONS_RELEVANT_ROLE",
            ][..],
        ),
        (
            "TEST",
            "architect",
            &[
                "CASE_HEADER",
                "PLAN_HEADER",
                "DECISIONS_GLOBAL_MIN",
                "SUBTASKS_ALL_MIN",
            ][..],
        ),
        (
            "TEST",
            "developer",
            &["CASE_HEADER", "PLAN_HEADER", "SUBTASKS_ROLE_MIN"][..],
        ),
        (
            "TEST",
            "devops",
            &["CASE_HEADER", "PLAN_HEADER", "SUBTASKS_ROLE_MIN"][..],
        ),
        (
            "TEST",
            "qa",
            &[
                "CASE_HEADER",
                "PLAN_HEADER",
                "DECISIONS_GLOBAL",
                "SUBTASKS_ALL",
                "MILESTONES",
            ][..],
        ),
        (
            "DOC",
            "architect",
            &[
                "CASE_HEADER",
                "PLAN_HEADER",
                "DECISIONS_GLOBAL",
                "SUMMARY_LAST",
            ][..],
        ),
        (
            "DOC",
            "qa",
            &[
                "CASE_HEADER",
                "PLAN_HEADER",
                "SUBTASKS_ALL_MIN",
                "MILESTONES",
            ][..],
        ),
    ];

    for (phase, role, expected) in cases {
        let expected = expected
            .iter()
            .map(|value| (*value).to_string())
            .collect::<Vec<_>>();
        assert_eq!(expected_scopes(phase, role), expected, "{phase}/{role}");
    }
}

#[test]
fn unknown_phase_or_role_returns_empty_scopes() {
    assert!(expected_scopes("BUILD", "DEV").is_empty());
    assert!(expected_scopes("UNKNOWN", "developer").is_empty());
    assert!(expected_scopes("DOC", "developer").is_empty());
}

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
