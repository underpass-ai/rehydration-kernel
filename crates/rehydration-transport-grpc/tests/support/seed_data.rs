#![allow(dead_code)]

use std::collections::BTreeMap;

use rehydration_domain::{
    PortError, ProjectionMutation, ProjectionWriter, RelationExplanation, RelationSemanticClass,
};
use rehydration_ports::{NodeDetailProjection, NodeProjection, NodeRelationProjection};

pub(crate) const ROOT_NODE_ID: &str = "story-123";
pub(crate) const ROOT_NODE_KIND: &str = "story";
pub(crate) const ROOT_TITLE: &str = "Hydrate projection shell";
pub(crate) const ROOT_SUMMARY: &str = "Root story summary";
pub(crate) const ROOT_STATUS: &str = "ACTIVE";
pub(crate) const ROOT_LABEL: &str = "Story";
pub(crate) const ROOT_CREATED_BY: &str = "planner";
pub(crate) const ROOT_PLAN_ID: &str = "plan-42";

pub(crate) const DECISION_ID: &str = "decision-1";
pub(crate) const DECISION_KIND: &str = "decision";
pub(crate) const DECISION_TITLE: &str = "Use beta kernel contract";
pub(crate) const DECISION_SUMMARY: &str = "Decision summary";
pub(crate) const DECISION_STATUS: &str = "ACCEPTED";
pub(crate) const DECISION_LABEL: &str = "Decision";

pub(crate) const TASK_ID: &str = "task-1";
pub(crate) const TASK_KIND: &str = "task";
pub(crate) const TASK_TITLE: &str = "Wire gRPC facade";
pub(crate) const TASK_SUMMARY: &str = "Task summary";
pub(crate) const TASK_STATUS: &str = "READY";
pub(crate) const TASK_LABEL: &str = "Task";
pub(crate) const TASK_ROLE: &str = "DEV";
pub(crate) const TASK_PRIORITY: &str = "3";

pub(crate) const DEVELOPER_ROLE: &str = "developer";
pub(crate) const BUILD_PHASE: &str = "BUILD";

pub(crate) const ROOT_DETAIL: &str = "Extended story detail";
pub(crate) const ROOT_DETAIL_HASH: &str = "hash-story";
pub(crate) const ROOT_DETAIL_REVISION: u64 = 2;
pub(crate) const DECISION_DETAIL: &str = "Detailed rationale";
pub(crate) const DECISION_DETAIL_HASH: &str = "hash-decision";
pub(crate) const DECISION_DETAIL_REVISION: u64 = 3;

pub(crate) const RECORDS_RELATION: &str = "records";
pub(crate) const HAS_TASK_RELATION: &str = "has_task";

pub(crate) const REHYDRATE_TIMELINE_EVENTS: i32 = 7;

pub(crate) async fn seed_projection_graph<W>(writer: &W) -> Result<(), PortError>
where
    W: ProjectionWriter + Send + Sync,
{
    writer.apply_mutations(graph_mutations()).await
}

pub(crate) async fn seed_node_details<W>(writer: &W) -> Result<(), PortError>
where
    W: ProjectionWriter + Send + Sync,
{
    writer.apply_mutations(node_detail_mutations()).await
}

pub(crate) fn allowed_validate_scope_request_scopes() -> Vec<String> {
    [
        "CASE_HEADER",
        "PLAN_HEADER",
        "SUBTASKS_ROLE",
        "DECISIONS_RELEVANT_ROLE",
        "DEPS_RELEVANT",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

pub(crate) fn rejected_validate_scope_request_scopes() -> Vec<String> {
    [
        "CASE_HEADER",
        "PLAN_HEADER",
        "SUBTASKS_ROLE",
        "INVALID_SCOPE",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn graph_mutations() -> Vec<ProjectionMutation> {
    vec![
        ProjectionMutation::UpsertNode(NodeProjection {
            node_id: ROOT_NODE_ID.to_string(),
            node_kind: ROOT_NODE_KIND.to_string(),
            title: ROOT_TITLE.to_string(),
            summary: ROOT_SUMMARY.to_string(),
            status: ROOT_STATUS.to_string(),
            labels: vec![ROOT_LABEL.to_string()],
            properties: BTreeMap::from([
                ("created_by".to_string(), ROOT_CREATED_BY.to_string()),
                ("plan_id".to_string(), ROOT_PLAN_ID.to_string()),
            ]),
            provenance: None,
        }),
        ProjectionMutation::UpsertNode(NodeProjection {
            node_id: DECISION_ID.to_string(),
            node_kind: DECISION_KIND.to_string(),
            title: DECISION_TITLE.to_string(),
            summary: DECISION_SUMMARY.to_string(),
            status: DECISION_STATUS.to_string(),
            labels: vec![DECISION_LABEL.to_string()],
            properties: BTreeMap::new(),
            provenance: None,
        }),
        ProjectionMutation::UpsertNode(NodeProjection {
            node_id: TASK_ID.to_string(),
            node_kind: TASK_KIND.to_string(),
            title: TASK_TITLE.to_string(),
            summary: TASK_SUMMARY.to_string(),
            status: TASK_STATUS.to_string(),
            labels: vec![TASK_LABEL.to_string()],
            properties: BTreeMap::from([
                ("role".to_string(), TASK_ROLE.to_string()),
                ("priority".to_string(), TASK_PRIORITY.to_string()),
            ]),
            provenance: None,
        }),
        ProjectionMutation::UpsertNodeRelation(NodeRelationProjection {
            source_node_id: ROOT_NODE_ID.to_string(),
            target_node_id: DECISION_ID.to_string(),
            relation_type: RECORDS_RELATION.to_string(),
            explanation: RelationExplanation::new(RelationSemanticClass::Structural)
                .with_sequence(1),
        }),
        ProjectionMutation::UpsertNodeRelation(NodeRelationProjection {
            source_node_id: ROOT_NODE_ID.to_string(),
            target_node_id: TASK_ID.to_string(),
            relation_type: HAS_TASK_RELATION.to_string(),
            explanation: RelationExplanation::new(RelationSemanticClass::Motivational)
                .with_sequence(2)
                .with_rationale("the task operationalizes the selected beta kernel approach"),
        }),
    ]
}

fn node_detail_mutations() -> Vec<ProjectionMutation> {
    vec![
        ProjectionMutation::UpsertNodeDetail(NodeDetailProjection {
            node_id: ROOT_NODE_ID.to_string(),
            detail: ROOT_DETAIL.to_string(),
            content_hash: ROOT_DETAIL_HASH.to_string(),
            revision: ROOT_DETAIL_REVISION,
        }),
        ProjectionMutation::UpsertNodeDetail(NodeDetailProjection {
            node_id: DECISION_ID.to_string(),
            detail: DECISION_DETAIL.to_string(),
            content_hash: DECISION_DETAIL_HASH.to_string(),
            revision: DECISION_DETAIL_REVISION,
        }),
    ]
}
