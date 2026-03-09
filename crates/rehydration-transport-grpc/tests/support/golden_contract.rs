use std::collections::HashMap;

use rehydration_proto::fleet_context_v1::{
    CaseHeader, Decision, GetContextResponse, GetGraphRelationshipsResponse, GraphNode,
    GraphRelationship, PlanHeader, PromptBlocks, RehydrateSessionResponse, RehydrationStats,
    RoleContextPack, Subtask, ValidateScopeResponse,
};

use crate::support::seed_data::{
    BUILD_PHASE, DECISION_DETAIL, DECISION_ID, DECISION_KIND, DECISION_LABEL, DECISION_STATUS,
    DECISION_SUMMARY, DECISION_TITLE, DEVELOPER_ROLE, HAS_TASK_RELATION, RECORDS_RELATION,
    ROOT_CREATED_BY, ROOT_DETAIL, ROOT_DETAIL_REVISION, ROOT_LABEL, ROOT_NODE_ID, ROOT_NODE_KIND,
    ROOT_PLAN_ID, ROOT_STATUS, ROOT_SUMMARY, ROOT_TITLE, TASK_ID, TASK_KIND, TASK_LABEL,
    TASK_PRIORITY, TASK_ROLE, TASK_STATUS, TASK_SUMMARY, TASK_TITLE,
    allowed_validate_scope_request_scopes,
};

pub(crate) fn expected_get_context_response() -> GetContextResponse {
    let context = expected_rendered_context();
    let scopes = sorted_build_scopes();
    GetContextResponse {
        context: context.clone(),
        token_count: context.split_whitespace().count() as i32,
        scopes: scopes.clone(),
        version: "rev-1".to_string(),
        blocks: Some(PromptBlocks {
            system: format!("role={DEVELOPER_ROLE}"),
            context,
            tools: format!("active_scopes={}", scopes.join(",")),
        }),
    }
}

pub(crate) fn expected_focused_get_context_response() -> GetContextResponse {
    let context = [
        format!("Node {ROOT_TITLE} ({ROOT_NODE_KIND}): {ROOT_SUMMARY}"),
        format!("Node {TASK_TITLE} ({TASK_KIND}): {TASK_SUMMARY}"),
        format!("Node {DECISION_TITLE} ({DECISION_KIND}): {DECISION_SUMMARY}"),
        format!("Relationship {ROOT_NODE_ID} --{HAS_TASK_RELATION}--> {TASK_ID}"),
        format!("Relationship {ROOT_NODE_ID} --{RECORDS_RELATION}--> {DECISION_ID}"),
        format!("Detail {ROOT_NODE_ID} [rev {ROOT_DETAIL_REVISION}]: {ROOT_DETAIL}"),
        format!("Detail {DECISION_ID} [rev 3]: {DECISION_DETAIL}"),
    ]
    .join("\n\n");
    let scopes = sorted_build_scopes();

    GetContextResponse {
        context: context.clone(),
        token_count: context.split_whitespace().count() as i32,
        scopes: scopes.clone(),
        version: "rev-1".to_string(),
        blocks: Some(PromptBlocks {
            system: format!("role={DEVELOPER_ROLE}"),
            context,
            tools: format!("active_scopes={}", scopes.join(",")),
        }),
    }
}

pub(crate) fn expected_budgeted_get_context_response() -> GetContextResponse {
    let context = format!("Node {ROOT_TITLE} ({ROOT_NODE_KIND}): {ROOT_SUMMARY}");
    let scopes = sorted_build_scopes();

    GetContextResponse {
        context: context.clone(),
        token_count: context.split_whitespace().count() as i32,
        scopes: scopes.clone(),
        version: "rev-1".to_string(),
        blocks: Some(PromptBlocks {
            system: format!("role={DEVELOPER_ROLE}"),
            context,
            tools: format!("active_scopes={}", scopes.join(",")),
        }),
    }
}

fn sorted_build_scopes() -> Vec<String> {
    let mut scopes = allowed_validate_scope_request_scopes();
    scopes.sort();
    scopes
}

pub(crate) fn expected_rehydrate_session_response(
    generated_at_ms: i64,
    timeline_events: i32,
) -> RehydrateSessionResponse {
    RehydrateSessionResponse {
        case_id: ROOT_NODE_ID.to_string(),
        generated_at_ms,
        packs: HashMap::from([(
            DEVELOPER_ROLE.to_string(),
            RoleContextPack {
                role: DEVELOPER_ROLE.to_string(),
                case_header: Some(CaseHeader {
                    case_id: ROOT_NODE_ID.to_string(),
                    title: ROOT_TITLE.to_string(),
                    description: ROOT_SUMMARY.to_string(),
                    status: ROOT_STATUS.to_string(),
                    created_at: String::new(),
                    created_by: ROOT_CREATED_BY.to_string(),
                }),
                plan_header: Some(PlanHeader {
                    plan_id: ROOT_PLAN_ID.to_string(),
                    version: 1,
                    status: ROOT_STATUS.to_string(),
                    total_subtasks: 1,
                    completed_subtasks: 0,
                }),
                subtasks: vec![Subtask {
                    subtask_id: TASK_ID.to_string(),
                    title: TASK_TITLE.to_string(),
                    description: TASK_SUMMARY.to_string(),
                    role: TASK_ROLE.to_string(),
                    status: TASK_STATUS.to_string(),
                    dependencies: Vec::new(),
                    priority: TASK_PRIORITY.parse().expect("task priority is valid"),
                }],
                decisions: vec![Decision {
                    id: DECISION_ID.to_string(),
                    title: DECISION_TITLE.to_string(),
                    rationale: DECISION_DETAIL.to_string(),
                    status: DECISION_STATUS.to_string(),
                    decided_by: String::new(),
                    decided_at: String::new(),
                }],
                decision_deps: Vec::new(),
                impacted: Vec::new(),
                milestones: Vec::new(),
                last_summary: ROOT_DETAIL.to_string(),
                token_budget_hint: 384,
            },
        )]),
        stats: Some(RehydrationStats {
            decisions: 1,
            decision_edges: 0,
            impacts: 0,
            events: timeline_events,
            roles: vec![DEVELOPER_ROLE.to_string()],
        }),
    }
}

pub(crate) fn expected_validate_scope_allowed_response() -> ValidateScopeResponse {
    ValidateScopeResponse {
        allowed: true,
        missing: Vec::new(),
        extra: Vec::new(),
        reason: "All scopes are allowed".to_string(),
    }
}

pub(crate) fn expected_validate_scope_rejected_response() -> ValidateScopeResponse {
    let _ = BUILD_PHASE;
    ValidateScopeResponse {
        allowed: false,
        missing: vec![
            "DECISIONS_RELEVANT_ROLE".to_string(),
            "DEPS_RELEVANT".to_string(),
        ],
        extra: vec!["INVALID_SCOPE".to_string()],
        reason: "Missing required scopes: DECISIONS_RELEVANT_ROLE, DEPS_RELEVANT; Extra scopes not allowed: INVALID_SCOPE".to_string(),
    }
}

pub(crate) fn expected_get_graph_relationships_response() -> GetGraphRelationshipsResponse {
    GetGraphRelationshipsResponse {
        node: Some(GraphNode {
            id: ROOT_NODE_ID.to_string(),
            labels: vec![ROOT_LABEL.to_string()],
            properties: HashMap::from([
                ("created_by".to_string(), ROOT_CREATED_BY.to_string()),
                ("plan_id".to_string(), ROOT_PLAN_ID.to_string()),
                ("summary".to_string(), ROOT_SUMMARY.to_string()),
                ("status".to_string(), ROOT_STATUS.to_string()),
            ]),
            r#type: ROOT_NODE_KIND.to_string(),
            title: ROOT_TITLE.to_string(),
        }),
        neighbors: vec![
            GraphNode {
                id: DECISION_ID.to_string(),
                labels: vec![DECISION_LABEL.to_string()],
                properties: HashMap::from([
                    ("summary".to_string(), DECISION_SUMMARY.to_string()),
                    ("status".to_string(), DECISION_STATUS.to_string()),
                ]),
                r#type: DECISION_KIND.to_string(),
                title: DECISION_TITLE.to_string(),
            },
            GraphNode {
                id: TASK_ID.to_string(),
                labels: vec![TASK_LABEL.to_string()],
                properties: HashMap::from([
                    ("priority".to_string(), TASK_PRIORITY.to_string()),
                    ("role".to_string(), TASK_ROLE.to_string()),
                    ("summary".to_string(), TASK_SUMMARY.to_string()),
                    ("status".to_string(), TASK_STATUS.to_string()),
                ]),
                r#type: TASK_KIND.to_string(),
                title: TASK_TITLE.to_string(),
            },
        ],
        relationships: vec![
            GraphRelationship {
                from_node_id: ROOT_NODE_ID.to_string(),
                to_node_id: DECISION_ID.to_string(),
                r#type: RECORDS_RELATION.to_string(),
                properties: HashMap::new(),
            },
            GraphRelationship {
                from_node_id: ROOT_NODE_ID.to_string(),
                to_node_id: TASK_ID.to_string(),
                r#type: HAS_TASK_RELATION.to_string(),
                properties: HashMap::new(),
            },
        ],
        success: true,
        message: String::new(),
    }
}

fn expected_rendered_context() -> String {
    [
        format!("Node {ROOT_TITLE} ({ROOT_NODE_KIND}): {ROOT_SUMMARY}"),
        format!("Node {DECISION_TITLE} ({DECISION_KIND}): {DECISION_SUMMARY}"),
        format!("Node {TASK_TITLE} ({TASK_KIND}): {TASK_SUMMARY}"),
        format!("Relationship {ROOT_NODE_ID} --{RECORDS_RELATION}--> {DECISION_ID}"),
        format!("Relationship {ROOT_NODE_ID} --{HAS_TASK_RELATION}--> {TASK_ID}"),
        format!("Detail {ROOT_NODE_ID} [rev {ROOT_DETAIL_REVISION}]: {ROOT_DETAIL}"),
        format!("Detail {DECISION_ID} [rev 3]: {DECISION_DETAIL}"),
    ]
    .join("\n\n")
}
