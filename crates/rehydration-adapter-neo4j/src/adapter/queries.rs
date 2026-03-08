pub(crate) const ROOT_QUERY: &str = "
MATCH (pack:RoleContextPackProjection {case_id: $case_id, role: $role})
RETURN coalesce(pack.latest_summary, '') AS latest_summary,
       coalesce(pack.token_budget_hint, 4096) AS token_budget_hint
LIMIT 1
";

pub(crate) const CASE_HEADER_QUERY: &str = "
MATCH (pack:RoleContextPackProjection {case_id: $case_id, role: $role})
MATCH (pack)-[:HAS_CASE_HEADER]->(case_header:CaseHeaderProjection)
RETURN case_header.case_id AS case_id,
       coalesce(case_header.title, '') AS title,
       coalesce(case_header.summary, '') AS summary,
       coalesce(case_header.status, '') AS status,
       coalesce(case_header.created_at, 0) AS created_at,
       coalesce(case_header.created_by, '') AS created_by
LIMIT 1
";

pub(crate) const PLAN_HEADER_QUERY: &str = "
MATCH (pack:RoleContextPackProjection {case_id: $case_id, role: $role})
MATCH (pack)-[:HAS_PLAN_HEADER]->(plan_header:PlanHeaderProjection)
RETURN plan_header.plan_id AS plan_id,
       coalesce(plan_header.revision, 0) AS revision,
       coalesce(plan_header.status, '') AS status,
       coalesce(plan_header.work_items_total, 0) AS work_items_total,
       coalesce(plan_header.work_items_completed, 0) AS work_items_completed
LIMIT 1
";

pub(crate) const WORK_ITEMS_QUERY: &str = "
MATCH (pack:RoleContextPackProjection {case_id: $case_id, role: $role})
MATCH (pack)-[:INCLUDES_WORK_ITEM]->(work_item:WorkItemProjection)
RETURN work_item.work_item_id AS work_item_id,
       coalesce(work_item.title, '') AS title,
       coalesce(work_item.summary, '') AS summary,
       coalesce(work_item.role, '') AS item_role,
       coalesce(work_item.phase, '') AS phase,
       coalesce(work_item.status, '') AS status,
       coalesce(work_item.dependency_ids, []) AS dependency_ids,
       coalesce(work_item.priority, 0) AS priority
ORDER BY priority DESC, work_item.work_item_id ASC
";

pub(crate) const DECISIONS_QUERY: &str = "
MATCH (pack:RoleContextPackProjection {case_id: $case_id, role: $role})
MATCH (pack)-[:INCLUDES_DECISION]->(decision:DecisionProjection)
RETURN decision.decision_id AS decision_id,
       coalesce(decision.title, '') AS title,
       coalesce(decision.rationale, '') AS rationale,
       coalesce(decision.status, '') AS status,
       coalesce(decision.owner, '') AS owner,
       coalesce(decision.decided_at, 0) AS decided_at
ORDER BY decided_at DESC, decision.decision_id ASC
";

pub(crate) const DECISION_RELATIONS_QUERY: &str = "
MATCH (pack:RoleContextPackProjection {case_id: $case_id, role: $role})
MATCH (pack)-[:HAS_DECISION_RELATION]->(decision_relation:DecisionRelationProjection)
RETURN decision_relation.source_decision_id AS source_decision_id,
       decision_relation.target_decision_id AS target_decision_id,
       coalesce(decision_relation.relation_type, '') AS relation_type
ORDER BY source_decision_id ASC, target_decision_id ASC, relation_type ASC
";

pub(crate) const IMPACTS_QUERY: &str = "
MATCH (pack:RoleContextPackProjection {case_id: $case_id, role: $role})
MATCH (pack)-[:HAS_TASK_IMPACT]->(task_impact:TaskImpactProjection)
RETURN task_impact.decision_id AS decision_id,
       task_impact.work_item_id AS work_item_id,
       coalesce(task_impact.title, '') AS title,
       coalesce(task_impact.impact_type, '') AS impact_type
ORDER BY decision_id ASC, work_item_id ASC
";

pub(crate) const MILESTONES_QUERY: &str = "
MATCH (pack:RoleContextPackProjection {case_id: $case_id, role: $role})
MATCH (pack)-[:HAS_MILESTONE]->(milestone:MilestoneProjection)
RETURN coalesce(milestone.milestone_type, '') AS milestone_type,
       coalesce(milestone.description, '') AS description,
       coalesce(milestone.occurred_at, 0) AS occurred_at,
       coalesce(milestone.actor, '') AS actor
ORDER BY occurred_at DESC, milestone_type ASC
";

pub(crate) const ROOT_NODE_QUERY: &str = "
MATCH (root:ProjectionNode {node_id: $root_node_id})
RETURN root.node_id AS node_id,
       coalesce(root.node_kind, '') AS node_kind,
       coalesce(root.title, '') AS title,
       coalesce(root.summary, '') AS summary,
       coalesce(root.status, '') AS status,
       coalesce(root.node_labels, []) AS node_labels,
       coalesce(root.properties_json, '{}') AS properties_json
LIMIT 1
";

pub(crate) const NODE_NEIGHBORHOOD_QUERY: &str = "
MATCH (root:ProjectionNode {node_id: $root_node_id})
OPTIONAL MATCH (source:ProjectionNode)-[edge:RELATED_TO]-(target:ProjectionNode)
WHERE source.node_id = $root_node_id OR target.node_id = $root_node_id
WITH source, edge, target,
     CASE WHEN source.node_id = $root_node_id THEN target ELSE source END AS neighbor
RETURN coalesce(neighbor.node_id, '') AS neighbor_node_id,
       coalesce(neighbor.node_kind, '') AS neighbor_node_kind,
       coalesce(neighbor.title, '') AS neighbor_title,
       coalesce(neighbor.summary, '') AS neighbor_summary,
       coalesce(neighbor.status, '') AS neighbor_status,
       coalesce(neighbor.node_labels, []) AS neighbor_node_labels,
       coalesce(neighbor.properties_json, '{}') AS neighbor_properties_json,
       coalesce(source.node_id, '') AS source_node_id,
       coalesce(target.node_id, '') AS target_node_id,
       coalesce(edge.relation_type, '') AS relation_type
";
