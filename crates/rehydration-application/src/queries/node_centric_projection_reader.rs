use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, SystemTime};

use rehydration_domain::{
    CaseHeader, CaseId, Decision, DecisionRelation, GraphNeighborhoodReader, NodeDetailProjection,
    NodeDetailReader, NodeNeighborhood, NodeProjection, NodeRelationProjection, PlanHeader, Role,
    RoleContextPack, WorkItem,
};

use crate::ApplicationError;

#[derive(Debug, Clone)]
pub struct NodeCentricProjectionReader<G, D> {
    graph_reader: G,
    detail_reader: D,
}

impl<G, D> NodeCentricProjectionReader<G, D> {
    pub fn new(graph_reader: G, detail_reader: D) -> Self {
        Self {
            graph_reader,
            detail_reader,
        }
    }
}

impl<G, D> NodeCentricProjectionReader<G, D>
where
    G: GraphNeighborhoodReader + Send + Sync,
    D: NodeDetailReader + Send + Sync,
{
    pub async fn load_pack(
        &self,
        root_node_id: &str,
        role: &str,
    ) -> Result<Option<RoleContextPack>, ApplicationError> {
        let Some(neighborhood) = self.graph_reader.load_neighborhood(root_node_id).await? else {
            return Ok(None);
        };

        let detail_by_node_id = load_details(&self.detail_reader, &neighborhood).await?;
        Ok(Some(build_pack(
            CaseId::new(root_node_id)?,
            Role::new(role)?,
            neighborhood,
            &detail_by_node_id,
        )))
    }
}

async fn load_details<D>(
    detail_reader: &D,
    neighborhood: &NodeNeighborhood,
) -> Result<BTreeMap<String, NodeDetailProjection>, rehydration_domain::PortError>
where
    D: NodeDetailReader + Send + Sync,
{
    let mut detail_by_node_id = BTreeMap::new();
    for node in std::iter::once(&neighborhood.root).chain(neighborhood.neighbors.iter()) {
        if let Some(detail) = detail_reader.load_node_detail(&node.node_id).await? {
            detail_by_node_id.insert(node.node_id.clone(), detail);
        }
    }

    Ok(detail_by_node_id)
}

fn build_pack(
    case_id: CaseId,
    role: Role,
    neighborhood: NodeNeighborhood,
    detail_by_node_id: &BTreeMap<String, NodeDetailProjection>,
) -> RoleContextPack {
    let root = &neighborhood.root;
    let latest_summary = latest_summary(root, detail_by_node_id);
    let token_budget_hint =
        parse_u32_property(&root.properties, "token_budget_hint").unwrap_or(4096);
    let case_header = CaseHeader::new(
        case_id,
        non_empty_or_fallback(root.title.trim(), root.node_id.as_str()),
        latest_summary.clone(),
        non_empty_or_fallback(root.status.trim(), "STATUS_UNSPECIFIED"),
        parse_system_time_property(&root.properties),
        property_or_default(
            &root.properties,
            &["created_by", "owner", "actor"],
            "projection-query",
        ),
    );

    let all_nodes = std::iter::once(&neighborhood.root)
        .chain(neighborhood.neighbors.iter())
        .collect::<Vec<_>>();
    let decision_ids = all_nodes
        .iter()
        .filter(|node| classify_node(node).is_decision())
        .map(|node| node.node_id.clone())
        .collect::<BTreeSet<_>>();
    let milestone_ids = all_nodes
        .iter()
        .filter(|node| classify_node(node).is_milestone())
        .map(|node| node.node_id.clone())
        .collect::<BTreeSet<_>>();
    let work_item_ids = all_nodes
        .iter()
        .filter(|node| classify_node(node).is_work_item())
        .map(|node| node.node_id.clone())
        .collect::<BTreeSet<_>>();

    let work_item_dependencies = collect_work_item_dependencies(&neighborhood, &work_item_ids);
    let mut work_items = all_nodes
        .iter()
        .filter(|node| work_item_ids.contains(&node.node_id))
        .map(|node| {
            WorkItem::new(
                node.node_id.clone(),
                non_empty_or_fallback(node.title.trim(), node.node_id.as_str()),
                node_summary(node, detail_by_node_id),
                property_or_default(&node.properties, &["role"], role.as_str()),
                property_or_default(&node.properties, &["phase"], "PHASE_UNSPECIFIED"),
                non_empty_or_fallback(node.status.trim(), "STATUS_UNSPECIFIED"),
                work_item_dependencies
                    .get(&node.node_id)
                    .cloned()
                    .unwrap_or_default(),
                parse_u32_property(&node.properties, "priority").unwrap_or_default(),
            )
        })
        .collect::<Vec<_>>();
    work_items.sort_by(|left, right| {
        right
            .priority()
            .cmp(&left.priority())
            .then_with(|| left.work_item_id().cmp(right.work_item_id()))
    });

    let mut decisions = all_nodes
        .iter()
        .filter(|node| decision_ids.contains(&node.node_id))
        .map(|node| {
            Decision::new(
                node.node_id.clone(),
                non_empty_or_fallback(node.title.trim(), node.node_id.as_str()),
                node_summary(node, detail_by_node_id),
                non_empty_or_fallback(node.status.trim(), "STATUS_UNSPECIFIED"),
                property_or_default(
                    &node.properties,
                    &["owner", "created_by", "actor"],
                    role.as_str(),
                ),
                parse_system_time_property(&node.properties),
            )
        })
        .collect::<Vec<_>>();
    decisions.sort_by(|left, right| {
        right
            .decided_at()
            .cmp(&left.decided_at())
            .then_with(|| left.decision_id().cmp(right.decision_id()))
    });

    let mut milestones = all_nodes
        .iter()
        .filter(|node| milestone_ids.contains(&node.node_id))
        .map(|node| {
            rehydration_domain::Milestone::new(
                property_or_default(
                    &node.properties,
                    &["milestone_type", "type"],
                    node.node_kind.as_str(),
                ),
                node_summary(node, detail_by_node_id),
                parse_system_time_property(&node.properties),
                property_or_default(
                    &node.properties,
                    &["actor", "owner", "created_by"],
                    "projection-query",
                ),
            )
        })
        .collect::<Vec<_>>();
    milestones.sort_by(|left, right| {
        right
            .occurred_at()
            .cmp(&left.occurred_at())
            .then_with(|| left.milestone_type().cmp(right.milestone_type()))
    });

    let decision_relations = neighborhood
        .relations
        .iter()
        .filter(|relation| {
            decision_ids.contains(&relation.source_node_id)
                && decision_ids.contains(&relation.target_node_id)
        })
        .map(|relation| {
            DecisionRelation::new(
                relation.source_node_id.clone(),
                relation.target_node_id.clone(),
                relation.relation_type.clone(),
            )
        })
        .collect::<Vec<_>>();

    let impacts = neighborhood
        .relations
        .iter()
        .filter_map(|relation| {
            build_task_impact(relation, &decision_ids, &work_item_ids, &all_nodes)
        })
        .collect::<Vec<_>>();

    let plan_header = build_plan_header(root, &work_items);

    RoleContextPack::new(
        role,
        case_header,
        plan_header,
        work_items,
        decisions,
        decision_relations,
        impacts,
        milestones,
        latest_summary,
        token_budget_hint,
    )
}

fn build_plan_header(root: &NodeProjection, work_items: &[WorkItem]) -> Option<PlanHeader> {
    if work_items.is_empty() && !root.properties.contains_key("plan_id") {
        return None;
    }

    let work_items_total = work_items.len() as u32;
    let work_items_completed = work_items
        .iter()
        .filter(|item| matches!(item.status(), "DONE" | "COMPLETED" | "CLOSED"))
        .count() as u32;

    Some(PlanHeader::new(
        property_or_default(
            &root.properties,
            &["plan_id"],
            &format!("plan:{}", root.node_id),
        ),
        parse_u64_property(&root.properties, "plan_revision").unwrap_or(1),
        property_or_default(&root.properties, &["plan_status"], root.status.as_str()),
        parse_u32_property(&root.properties, "work_items_total").unwrap_or(work_items_total),
        parse_u32_property(&root.properties, "work_items_completed")
            .unwrap_or(work_items_completed),
    ))
}

fn build_task_impact(
    relation: &NodeRelationProjection,
    decision_ids: &BTreeSet<String>,
    work_item_ids: &BTreeSet<String>,
    all_nodes: &[&NodeProjection],
) -> Option<rehydration_domain::TaskImpact> {
    let (decision_id, work_item_id) = if decision_ids.contains(&relation.source_node_id)
        && work_item_ids.contains(&relation.target_node_id)
    {
        (
            relation.source_node_id.as_str(),
            relation.target_node_id.as_str(),
        )
    } else if decision_ids.contains(&relation.target_node_id)
        && work_item_ids.contains(&relation.source_node_id)
    {
        (
            relation.target_node_id.as_str(),
            relation.source_node_id.as_str(),
        )
    } else {
        return None;
    };

    let work_item_title = all_nodes
        .iter()
        .find(|node| node.node_id == work_item_id)
        .map(|node| non_empty_or_fallback(node.title.trim(), work_item_id))
        .unwrap_or_else(|| work_item_id.to_string());

    Some(rehydration_domain::TaskImpact::new(
        decision_id,
        work_item_id,
        work_item_title,
        relation.relation_type.clone(),
    ))
}

fn collect_work_item_dependencies(
    neighborhood: &NodeNeighborhood,
    work_item_ids: &BTreeSet<String>,
) -> BTreeMap<String, Vec<String>> {
    let mut dependencies = BTreeMap::<String, Vec<String>>::new();

    for relation in &neighborhood.relations {
        if work_item_ids.contains(&relation.source_node_id)
            && work_item_ids.contains(&relation.target_node_id)
        {
            dependencies
                .entry(relation.source_node_id.clone())
                .or_default()
                .push(relation.target_node_id.clone());
        }
    }

    dependencies
}

fn latest_summary(
    root: &NodeProjection,
    detail_by_node_id: &BTreeMap<String, NodeDetailProjection>,
) -> String {
    property_or_option(&root.properties, &["latest_summary"])
        .or_else(|| {
            non_empty(
                detail_by_node_id
                    .get(&root.node_id)
                    .map(|detail| detail.detail.as_str()),
            )
        })
        .or_else(|| non_empty(Some(root.summary.as_str())))
        .unwrap_or_else(|| root.title.clone())
}

fn node_summary(
    node: &NodeProjection,
    detail_by_node_id: &BTreeMap<String, NodeDetailProjection>,
) -> String {
    detail_by_node_id
        .get(&node.node_id)
        .and_then(|detail| non_empty(Some(detail.detail.as_str())))
        .or_else(|| non_empty(Some(node.summary.as_str())))
        .unwrap_or_else(|| node.title.clone())
}

fn property_or_default(
    properties: &BTreeMap<String, String>,
    keys: &[&str],
    default: &str,
) -> String {
    property_or_option(properties, keys).unwrap_or_else(|| default.to_string())
}

fn property_or_option(properties: &BTreeMap<String, String>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| properties.get(*key))
        .and_then(|value| non_empty(Some(value.as_str())))
}

fn non_empty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn non_empty_or_fallback(value: &str, fallback: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        fallback.to_string()
    } else {
        value.to_string()
    }
}

fn parse_u32_property(properties: &BTreeMap<String, String>, key: &str) -> Option<u32> {
    properties
        .get(key)
        .and_then(|value| value.parse::<u32>().ok())
}

fn parse_u64_property(properties: &BTreeMap<String, String>, key: &str) -> Option<u64> {
    properties
        .get(key)
        .and_then(|value| value.parse::<u64>().ok())
}

fn parse_system_time_property(properties: &BTreeMap<String, String>) -> SystemTime {
    for key in [
        "occurred_at",
        "decided_at",
        "created_at",
        "created_at_millis",
    ] {
        if let Some(value) = properties
            .get(key)
            .and_then(|value| value.parse::<u64>().ok())
        {
            return SystemTime::UNIX_EPOCH + Duration::from_millis(value);
        }
    }
    SystemTime::UNIX_EPOCH
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NodeClass {
    Decision,
    Milestone,
    WorkItem,
}
impl NodeClass {
    const fn is_decision(self) -> bool {
        matches!(self, Self::Decision)
    }
    const fn is_milestone(self) -> bool {
        matches!(self, Self::Milestone)
    }
    const fn is_work_item(self) -> bool {
        matches!(self, Self::WorkItem)
    }
}

fn classify_node(node: &NodeProjection) -> NodeClass {
    let signals = std::iter::once(node.node_kind.to_ascii_lowercase())
        .chain(node.labels.iter().map(|label| label.to_ascii_lowercase()))
        .collect::<Vec<_>>();

    if signals.iter().any(|signal| signal.contains("decision")) {
        return NodeClass::Decision;
    }
    if signals
        .iter()
        .any(|signal| signal.contains("milestone") || signal.contains("phase"))
    {
        return NodeClass::Milestone;
    }
    NodeClass::WorkItem
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use rehydration_domain::{
        NodeDetailProjection, NodeNeighborhood, NodeProjection, NodeRelationProjection,
    };
    use rehydration_testkit::{InMemoryGraphNeighborhoodReader, InMemoryNodeDetailReader};

    use super::NodeCentricProjectionReader;

    #[tokio::test]
    async fn load_pack_rehydrates_from_graph_and_detail() {
        let graph_reader = InMemoryGraphNeighborhoodReader::with_neighborhood(NodeNeighborhood {
            root: NodeProjection {
                node_id: "node-root".to_string(),
                node_kind: "story".to_string(),
                title: "Node Root".to_string(),
                summary: "Root summary".to_string(),
                status: "ACTIVE".to_string(),
                labels: vec!["story".to_string()],
                properties: BTreeMap::from([
                    ("plan_id".to_string(), "plan-123".to_string()),
                    ("token_budget_hint".to_string(), "2048".to_string()),
                ]),
            },
            neighbors: vec![
                NodeProjection {
                    node_id: "decision-1".to_string(),
                    node_kind: "decision".to_string(),
                    title: "Use node-centric projection".to_string(),
                    summary: "Decision summary".to_string(),
                    status: "ACCEPTED".to_string(),
                    labels: vec!["decision".to_string()],
                    properties: BTreeMap::new(),
                },
                NodeProjection {
                    node_id: "task-1".to_string(),
                    node_kind: "task".to_string(),
                    title: "Refactor application layer".to_string(),
                    summary: "Refine the CQRS boundary".to_string(),
                    status: "READY".to_string(),
                    labels: vec!["task".to_string()],
                    properties: BTreeMap::from([("priority".to_string(), "3".to_string())]),
                },
            ],
            relations: vec![
                NodeRelationProjection {
                    source_node_id: "decision-1".to_string(),
                    target_node_id: "task-1".to_string(),
                    relation_type: "DIRECT".to_string(),
                },
                NodeRelationProjection {
                    source_node_id: "task-1".to_string(),
                    target_node_id: "decision-1".to_string(),
                    relation_type: "DEPENDS_ON".to_string(),
                },
            ],
        });
        let detail_reader = InMemoryNodeDetailReader::with_details([NodeDetailProjection {
            node_id: "decision-1".to_string(),
            detail: "Decision detail from Valkey".to_string(),
            content_hash: "hash-1".to_string(),
            revision: 7,
        }]);
        let reader = NodeCentricProjectionReader::new(graph_reader, detail_reader);

        let pack = reader
            .load_pack("node-root", "developer")
            .await
            .expect("load must succeed")
            .expect("pack should exist");

        assert_eq!(pack.case_header().case_id().as_str(), "node-root");
        assert_eq!(
            pack.decisions()[0].rationale(),
            "Decision detail from Valkey"
        );
        assert_eq!(pack.impacts()[0].impact_type(), "DIRECT");
        assert_eq!(pack.work_items()[0].priority(), 3);
        assert_eq!(pack.token_budget_hint(), 2048);
    }
}
