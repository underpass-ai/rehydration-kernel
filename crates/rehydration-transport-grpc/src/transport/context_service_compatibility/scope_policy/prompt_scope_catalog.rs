type ScopeGroup = &'static [&'static str];

const CASE_PLAN: ScopeGroup = &["CASE_HEADER", "PLAN_HEADER"];
const DECISIONS_GLOBAL: ScopeGroup = &["DECISIONS_GLOBAL"];
const DECISIONS_GLOBAL_MIN: ScopeGroup = &["DECISIONS_GLOBAL_MIN"];
const DECISIONS_RELEVANT_ROLE: ScopeGroup = &["DECISIONS_RELEVANT_ROLE"];
const DEPS_DECISIONS: ScopeGroup = &["DEPS_DECISIONS"];
const DEPS_RELEVANT: ScopeGroup = &["DEPS_RELEVANT"];
const MILESTONES: ScopeGroup = &["MILESTONES"];
const SUBTASKS_ALL: ScopeGroup = &["SUBTASKS_ALL"];
const SUBTASKS_ALL_MIN: ScopeGroup = &["SUBTASKS_ALL_MIN"];
const SUBTASKS_ROLE: ScopeGroup = &["SUBTASKS_ROLE"];
const SUBTASKS_ROLE_MIN: ScopeGroup = &["SUBTASKS_ROLE_MIN"];
const SUMMARY_LAST: ScopeGroup = &["SUMMARY_LAST"];

pub(crate) fn expected_scopes(phase: &str, role: &str) -> Vec<String> {
    match phase {
        "DESIGN" => design_scopes(role),
        "BUILD" => build_scopes(role),
        "TEST" => test_scopes(role),
        "DOC" => doc_scopes(role),
        _ => Vec::new(),
    }
}

fn design_scopes(role: &str) -> Vec<String> {
    let mut scopes = case_plan_scopes();

    match role {
        "architect" => {
            push_global_decision_scopes(&mut scopes);
            push_group(&mut scopes, MILESTONES);
            push_group(&mut scopes, SUMMARY_LAST);
        }
        "developer" => {
            push_relevant_decision_scopes(&mut scopes);
            push_group(&mut scopes, SUMMARY_LAST);
        }
        "devops" => {
            push_relevant_decision_scopes(&mut scopes);
            push_group(&mut scopes, MILESTONES);
        }
        "qa" => {
            push_group(&mut scopes, DECISIONS_GLOBAL);
            push_group(&mut scopes, SUBTASKS_ALL);
            push_group(&mut scopes, MILESTONES);
            push_group(&mut scopes, SUMMARY_LAST);
        }
        "data" => push_relevant_decision_scopes(&mut scopes),
        _ => return Vec::new(),
    }

    scopes
}

fn build_scopes(role: &str) -> Vec<String> {
    let mut scopes = case_plan_scopes();

    match role {
        "architect" => {
            push_global_decision_scopes(&mut scopes);
            push_group(&mut scopes, SUBTASKS_ALL);
            push_group(&mut scopes, MILESTONES);
        }
        "developer" | "devops" => push_role_execution_scopes(&mut scopes),
        "qa" => {
            push_group(&mut scopes, SUBTASKS_ALL_MIN);
            push_group(&mut scopes, DECISIONS_GLOBAL_MIN);
            push_group(&mut scopes, MILESTONES);
        }
        "data" => {
            push_group(&mut scopes, SUBTASKS_ROLE);
            push_group(&mut scopes, DECISIONS_RELEVANT_ROLE);
        }
        _ => return Vec::new(),
    }

    scopes
}

fn test_scopes(role: &str) -> Vec<String> {
    let mut scopes = case_plan_scopes();

    match role {
        "architect" => {
            push_group(&mut scopes, DECISIONS_GLOBAL_MIN);
            push_group(&mut scopes, SUBTASKS_ALL_MIN);
        }
        "developer" | "devops" => push_group(&mut scopes, SUBTASKS_ROLE_MIN),
        "qa" => {
            push_group(&mut scopes, DECISIONS_GLOBAL);
            push_group(&mut scopes, SUBTASKS_ALL);
            push_group(&mut scopes, MILESTONES);
        }
        _ => return Vec::new(),
    }

    scopes
}

fn doc_scopes(role: &str) -> Vec<String> {
    let mut scopes = case_plan_scopes();

    match role {
        "architect" => {
            push_group(&mut scopes, DECISIONS_GLOBAL);
            push_group(&mut scopes, SUMMARY_LAST);
        }
        "qa" => {
            push_group(&mut scopes, SUBTASKS_ALL_MIN);
            push_group(&mut scopes, MILESTONES);
        }
        _ => return Vec::new(),
    }

    scopes
}

fn case_plan_scopes() -> Vec<String> {
    let mut scopes = Vec::new();
    push_group(&mut scopes, CASE_PLAN);
    scopes
}

fn push_global_decision_scopes(scopes: &mut Vec<String>) {
    push_group(scopes, DECISIONS_GLOBAL);
    push_group(scopes, DEPS_DECISIONS);
}

fn push_relevant_decision_scopes(scopes: &mut Vec<String>) {
    push_group(scopes, DECISIONS_RELEVANT_ROLE);
    push_group(scopes, DEPS_RELEVANT);
}

fn push_role_execution_scopes(scopes: &mut Vec<String>) {
    push_group(scopes, SUBTASKS_ROLE);
    push_relevant_decision_scopes(scopes);
}

fn push_group(scopes: &mut Vec<String>, group: ScopeGroup) {
    scopes.extend(group.iter().map(|value| (*value).to_string()));
}
