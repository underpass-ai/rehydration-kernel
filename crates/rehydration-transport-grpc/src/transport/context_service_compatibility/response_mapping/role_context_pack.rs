use std::collections::{BTreeMap, BTreeSet};

use rehydration_domain::{BundleNode, BundleNodeDetail, BundleRelationship, RehydrationBundle};
use rehydration_proto::fleet_context_v1::{
    CaseHeader, Decision, DecisionRelation, ImpactedSubtask, Milestone, PlanHeader,
    RoleContextPack, Subtask,
};

pub(crate) fn proto_role_context_pack(bundle: &RehydrationBundle) -> RoleContextPack {
    let detail_by_node_id = bundle
        .node_details()
        .iter()
        .map(|detail| (detail.node_id(), detail))
        .collect::<BTreeMap<_, _>>();
    let node_by_id = bundle
        .neighbor_nodes()
        .iter()
        .map(|node| (node.node_id(), node))
        .chain(std::iter::once((
            bundle.root_node().node_id(),
            bundle.root_node(),
        )))
        .collect::<BTreeMap<_, _>>();

    let subtasks = bundle
        .neighbor_nodes()
        .iter()
        .filter(|node| is_task_like(node.node_kind()))
        .map(|node| proto_subtask(node, bundle.relationships()))
        .collect::<Vec<_>>();
    let decisions = bundle
        .neighbor_nodes()
        .iter()
        .filter(|node| is_decision_like(node.node_kind()))
        .map(|node| proto_decision(node, detail_by_node_id.get(node.node_id()).copied()))
        .collect::<Vec<_>>();
    let decision_ids = decisions
        .iter()
        .map(|decision| decision.id.clone())
        .collect::<BTreeSet<_>>();

    RoleContextPack {
        role: bundle.role().as_str().to_string(),
        case_header: Some(proto_case_header(bundle)),
        plan_header: Some(proto_plan_header(bundle, &subtasks)),
        subtasks,
        decisions,
        decision_deps: bundle
            .relationships()
            .iter()
            .filter(|relationship| {
                decision_ids.contains(relationship.source_node_id())
                    && decision_ids.contains(relationship.target_node_id())
            })
            .map(proto_decision_relation)
            .collect(),
        impacted: bundle
            .relationships()
            .iter()
            .filter_map(|relationship| proto_impacted_subtask(relationship, &node_by_id))
            .collect(),
        milestones: Vec::<Milestone>::new(),
        last_summary: detail_by_node_id
            .get(bundle.root_node().node_id())
            .map(|detail| detail.detail().to_string())
            .unwrap_or_default(),
        token_budget_hint: estimate_token_budget(bundle),
    }
}

fn proto_case_header(bundle: &RehydrationBundle) -> CaseHeader {
    let root = bundle.root_node();
    CaseHeader {
        case_id: root.node_id().to_string(),
        title: root.title().to_string(),
        description: root.summary().to_string(),
        status: root.status().to_string(),
        created_at: root
            .properties()
            .get("created_at")
            .cloned()
            .unwrap_or_default(),
        created_by: root
            .properties()
            .get("created_by")
            .cloned()
            .unwrap_or_default(),
    }
}

fn proto_plan_header(bundle: &RehydrationBundle, subtasks: &[Subtask]) -> PlanHeader {
    PlanHeader {
        plan_id: bundle
            .root_node()
            .properties()
            .get("plan_id")
            .cloned()
            .unwrap_or_else(|| bundle.root_node().node_id().to_string()),
        version: bundle.metadata().revision.min(i32::MAX as u64) as i32,
        status: bundle.root_node().status().to_string(),
        total_subtasks: subtasks.len().min(i32::MAX as usize) as i32,
        completed_subtasks: subtasks
            .iter()
            .filter(|subtask| matches!(subtask.status.as_str(), "DONE" | "COMPLETED"))
            .count()
            .min(i32::MAX as usize) as i32,
    }
}

fn proto_subtask(node: &BundleNode, relationships: &[BundleRelationship]) -> Subtask {
    let dependencies = relationships
        .iter()
        .filter(|relationship| relationship.target_node_id() == node.node_id())
        .filter(|relationship| relationship.relationship_type().contains("DEPENDS"))
        .map(|relationship| relationship.source_node_id().to_string())
        .collect();

    Subtask {
        subtask_id: node.node_id().to_string(),
        title: node.title().to_string(),
        description: node.summary().to_string(),
        role: node.properties().get("role").cloned().unwrap_or_default(),
        status: node.status().to_string(),
        dependencies,
        priority: node
            .properties()
            .get("priority")
            .and_then(|value| value.parse::<i32>().ok())
            .unwrap_or_default(),
    }
}

fn proto_decision(node: &BundleNode, detail: Option<&BundleNodeDetail>) -> Decision {
    Decision {
        id: node.node_id().to_string(),
        title: node.title().to_string(),
        rationale: detail
            .map(|value| value.detail().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| node.summary().to_string()),
        status: node.status().to_string(),
        decided_by: node
            .properties()
            .get("decided_by")
            .cloned()
            .unwrap_or_default(),
        decided_at: node
            .properties()
            .get("decided_at")
            .cloned()
            .unwrap_or_default(),
    }
}

fn proto_decision_relation(relationship: &BundleRelationship) -> DecisionRelation {
    DecisionRelation {
        src_id: relationship.source_node_id().to_string(),
        dst_id: relationship.target_node_id().to_string(),
        relation_type: relationship.relationship_type().to_string(),
    }
}

fn proto_impacted_subtask(
    relationship: &BundleRelationship,
    node_by_id: &BTreeMap<&str, &BundleNode>,
) -> Option<ImpactedSubtask> {
    let source = node_by_id.get(relationship.source_node_id())?;
    let target = node_by_id.get(relationship.target_node_id())?;

    if is_decision_like(source.node_kind()) && is_task_like(target.node_kind()) {
        return Some(ImpactedSubtask {
            decision_id: source.node_id().to_string(),
            subtask_id: target.node_id().to_string(),
            title: target.title().to_string(),
        });
    }
    if is_decision_like(target.node_kind()) && is_task_like(source.node_kind()) {
        return Some(ImpactedSubtask {
            decision_id: target.node_id().to_string(),
            subtask_id: source.node_id().to_string(),
            title: source.title().to_string(),
        });
    }

    None
}

fn is_task_like(node_kind: &str) -> bool {
    matches!(
        node_kind.to_ascii_lowercase().as_str(),
        "task" | "subtask" | "work_item"
    )
}

fn is_decision_like(node_kind: &str) -> bool {
    node_kind.eq_ignore_ascii_case("decision")
}

fn estimate_token_budget(bundle: &RehydrationBundle) -> i32 {
    let estimated = bundle.stats().selected_nodes() * 128;
    estimated.min(i32::MAX as u32) as i32
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use rehydration_domain::{
        BundleMetadata, BundleNode, BundleNodeDetail, BundleRelationship, CaseId,
        RehydrationBundle, RelationExplanation, RelationSemanticClass, Role,
    };

    use super::proto_role_context_pack;

    #[test]
    fn role_context_pack_derives_external_legacy_shape_from_bundle() {
        let bundle = RehydrationBundle::new(
            CaseId::new("case-123").expect("case id"),
            Role::new("developer").expect("role"),
            BundleNode::new(
                "case-123",
                "story",
                "Story",
                "Root summary",
                "ACTIVE",
                vec!["Story".to_string()],
                BTreeMap::from([("plan_id".to_string(), "plan-1".to_string())]),
            ),
            vec![
                BundleNode::new(
                    "task-1",
                    "task",
                    "Task",
                    "Task summary",
                    "COMPLETED",
                    vec!["Task".to_string()],
                    BTreeMap::from([
                        ("role".to_string(), "DEV".to_string()),
                        ("priority".to_string(), "3".to_string()),
                    ]),
                ),
                BundleNode::new(
                    "decision-1",
                    "decision",
                    "Decision",
                    "Decision summary",
                    "ACCEPTED",
                    vec!["Decision".to_string()],
                    BTreeMap::new(),
                ),
            ],
            vec![BundleRelationship::new(
                "decision-1",
                "task-1",
                "IMPACTS",
                RelationExplanation::new(RelationSemanticClass::Causal),
            )],
            vec![BundleNodeDetail::new(
                "decision-1",
                "Detailed rationale",
                "hash-1",
                2,
            )],
            BundleMetadata::initial("0.1.0"),
        )
        .expect("bundle");

        let pack = proto_role_context_pack(&bundle);

        assert_eq!(pack.role, "developer");
        assert_eq!(pack.subtasks.len(), 1);
        assert_eq!(pack.decisions.len(), 1);
        assert_eq!(pack.impacted.len(), 1);
        assert_eq!(pack.plan_header.expect("plan header").plan_id, "plan-1");
        assert_eq!(pack.decisions[0].rationale, "Detailed rationale");
    }
}
