pub(crate) fn expected_scopes(phase: &str, role: &str) -> Vec<String> {
    match (phase, role) {
        ("DESIGN", "architect") => scopes(&[
            "CASE_HEADER",
            "PLAN_HEADER",
            "DECISIONS_GLOBAL",
            "DEPS_DECISIONS",
            "MILESTONES",
            "SUMMARY_LAST",
        ]),
        ("DESIGN", "developer") => scopes(&[
            "CASE_HEADER",
            "PLAN_HEADER",
            "DECISIONS_RELEVANT_ROLE",
            "DEPS_RELEVANT",
            "SUMMARY_LAST",
        ]),
        ("DESIGN", "devops") => scopes(&[
            "CASE_HEADER",
            "PLAN_HEADER",
            "DECISIONS_RELEVANT_ROLE",
            "DEPS_RELEVANT",
            "MILESTONES",
        ]),
        ("DESIGN", "qa") => scopes(&[
            "CASE_HEADER",
            "PLAN_HEADER",
            "DECISIONS_GLOBAL",
            "SUBTASKS_ALL",
            "MILESTONES",
            "SUMMARY_LAST",
        ]),
        ("DESIGN", "data") => scopes(&[
            "CASE_HEADER",
            "PLAN_HEADER",
            "DECISIONS_RELEVANT_ROLE",
            "DEPS_RELEVANT",
        ]),
        ("BUILD", "architect") => scopes(&[
            "CASE_HEADER",
            "PLAN_HEADER",
            "DECISIONS_GLOBAL",
            "DEPS_DECISIONS",
            "SUBTASKS_ALL",
            "MILESTONES",
        ]),
        ("BUILD", "developer") => scopes(&[
            "CASE_HEADER",
            "PLAN_HEADER",
            "SUBTASKS_ROLE",
            "DECISIONS_RELEVANT_ROLE",
            "DEPS_RELEVANT",
        ]),
        ("BUILD", "devops") => scopes(&[
            "CASE_HEADER",
            "PLAN_HEADER",
            "SUBTASKS_ROLE",
            "DECISIONS_RELEVANT_ROLE",
            "DEPS_RELEVANT",
        ]),
        ("BUILD", "qa") => scopes(&[
            "CASE_HEADER",
            "PLAN_HEADER",
            "SUBTASKS_ALL_MIN",
            "DECISIONS_GLOBAL_MIN",
            "MILESTONES",
        ]),
        ("BUILD", "data") => scopes(&[
            "CASE_HEADER",
            "PLAN_HEADER",
            "SUBTASKS_ROLE",
            "DECISIONS_RELEVANT_ROLE",
        ]),
        ("TEST", "architect") => scopes(&[
            "CASE_HEADER",
            "PLAN_HEADER",
            "DECISIONS_GLOBAL_MIN",
            "SUBTASKS_ALL_MIN",
        ]),
        ("TEST", "developer") => scopes(&["CASE_HEADER", "PLAN_HEADER", "SUBTASKS_ROLE_MIN"]),
        ("TEST", "devops") => scopes(&["CASE_HEADER", "PLAN_HEADER", "SUBTASKS_ROLE_MIN"]),
        ("TEST", "qa") => scopes(&[
            "CASE_HEADER",
            "PLAN_HEADER",
            "DECISIONS_GLOBAL",
            "SUBTASKS_ALL",
            "MILESTONES",
        ]),
        ("DOC", "architect") => scopes(&[
            "CASE_HEADER",
            "PLAN_HEADER",
            "DECISIONS_GLOBAL",
            "SUMMARY_LAST",
        ]),
        ("DOC", "qa") => scopes(&[
            "CASE_HEADER",
            "PLAN_HEADER",
            "SUBTASKS_ALL_MIN",
            "MILESTONES",
        ]),
        _ => Vec::new(),
    }
}

fn scopes(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::expected_scopes;

    #[test]
    fn expected_scopes_match_build_developer_catalog() {
        assert_eq!(
            expected_scopes("BUILD", "developer"),
            vec![
                "CASE_HEADER".to_string(),
                "PLAN_HEADER".to_string(),
                "SUBTASKS_ROLE".to_string(),
                "DECISIONS_RELEVANT_ROLE".to_string(),
                "DEPS_RELEVANT".to_string(),
            ]
        );
    }

    #[test]
    fn unknown_phase_or_role_returns_empty_scopes() {
        assert!(expected_scopes("BUILD", "DEV").is_empty());
        assert!(expected_scopes("UNKNOWN", "developer").is_empty());
    }
}
