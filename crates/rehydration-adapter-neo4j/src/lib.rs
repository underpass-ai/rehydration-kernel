use std::fmt;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use neo4rs::{Graph, Query, Row, query};
use rehydration_domain::{
    CaseHeader, CaseId, Decision, DecisionRelation, Milestone, PlanHeader, Role, RoleContextPack,
    TaskImpact, WorkItem,
};
use rehydration_ports::{PortError, ProjectionReader};
use tokio::sync::OnceCell;

const ROOT_QUERY: &str = "
MATCH (pack:RoleContextPackProjection {case_id: $case_id, role: $role})
RETURN coalesce(pack.latest_summary, '') AS latest_summary,
       coalesce(pack.token_budget_hint, 4096) AS token_budget_hint
LIMIT 1
";

const CASE_HEADER_QUERY: &str = "
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

const PLAN_HEADER_QUERY: &str = "
MATCH (pack:RoleContextPackProjection {case_id: $case_id, role: $role})
MATCH (pack)-[:HAS_PLAN_HEADER]->(plan_header:PlanHeaderProjection)
RETURN plan_header.plan_id AS plan_id,
       coalesce(plan_header.revision, 0) AS revision,
       coalesce(plan_header.status, '') AS status,
       coalesce(plan_header.work_items_total, 0) AS work_items_total,
       coalesce(plan_header.work_items_completed, 0) AS work_items_completed
LIMIT 1
";

const WORK_ITEMS_QUERY: &str = "
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

const DECISIONS_QUERY: &str = "
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

const DECISION_RELATIONS_QUERY: &str = "
MATCH (pack:RoleContextPackProjection {case_id: $case_id, role: $role})
MATCH (pack)-[:HAS_DECISION_RELATION]->(decision_relation:DecisionRelationProjection)
RETURN decision_relation.source_decision_id AS source_decision_id,
       decision_relation.target_decision_id AS target_decision_id,
       coalesce(decision_relation.relation_type, '') AS relation_type
ORDER BY source_decision_id ASC, target_decision_id ASC, relation_type ASC
";

const IMPACTS_QUERY: &str = "
MATCH (pack:RoleContextPackProjection {case_id: $case_id, role: $role})
MATCH (pack)-[:HAS_TASK_IMPACT]->(task_impact:TaskImpactProjection)
RETURN task_impact.decision_id AS decision_id,
       task_impact.work_item_id AS work_item_id,
       coalesce(task_impact.title, '') AS title,
       coalesce(task_impact.impact_type, '') AS impact_type
ORDER BY decision_id ASC, work_item_id ASC
";

const MILESTONES_QUERY: &str = "
MATCH (pack:RoleContextPackProjection {case_id: $case_id, role: $role})
MATCH (pack)-[:HAS_MILESTONE]->(milestone:MilestoneProjection)
RETURN coalesce(milestone.milestone_type, '') AS milestone_type,
       coalesce(milestone.description, '') AS description,
       coalesce(milestone.occurred_at, 0) AS occurred_at,
       coalesce(milestone.actor, '') AS actor
ORDER BY occurred_at DESC, milestone_type ASC
";

#[derive(Clone)]
pub struct Neo4jProjectionReader {
    endpoint: Neo4jEndpoint,
    graph: Arc<OnceCell<Arc<Graph>>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Neo4jEndpoint {
    connection_uri: String,
    user: String,
    password: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProjectionRootRecord {
    latest_summary: String,
    token_budget_hint: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RawProjectionRootRecord {
    latest_summary: String,
    token_budget_hint: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RawCaseHeaderRecord {
    case_id: String,
    title: String,
    summary: String,
    status: String,
    created_at_millis: i64,
    created_by: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RawPlanHeaderRecord {
    plan_id: String,
    revision: i64,
    status: String,
    work_items_total: i64,
    work_items_completed: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RawWorkItemRecord {
    work_item_id: String,
    title: String,
    summary: String,
    role: String,
    phase: String,
    status: String,
    dependency_ids: Vec<String>,
    priority: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RawDecisionRecord {
    decision_id: String,
    title: String,
    rationale: String,
    status: String,
    owner: String,
    decided_at_millis: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RawMilestoneRecord {
    milestone_type: String,
    description: String,
    occurred_at_millis: i64,
    actor: String,
}

impl fmt::Debug for Neo4jProjectionReader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Neo4jProjectionReader")
            .field("endpoint", &self.endpoint)
            .field("connected", &self.graph.get().is_some())
            .finish()
    }
}

impl Neo4jProjectionReader {
    pub fn new(graph_uri: impl Into<String>) -> Result<Self, PortError> {
        let endpoint = Neo4jEndpoint::parse(graph_uri.into())?;
        Ok(Self {
            endpoint,
            graph: Arc::new(OnceCell::new()),
        })
    }

    async fn graph(&self) -> Result<Arc<Graph>, PortError> {
        let graph = self
            .graph
            .get_or_try_init(|| async {
                Graph::new(
                    &self.endpoint.connection_uri,
                    &self.endpoint.user,
                    &self.endpoint.password,
                )
                .await
                .map(Arc::new)
                .map_err(|error| {
                    PortError::Unavailable(format!(
                        "neo4j connection failed for `{}`: {error}",
                        self.endpoint.connection_uri
                    ))
                })
            })
            .await?;

        Ok(Arc::clone(graph))
    }

    async fn load_root(
        &self,
        graph: &Graph,
        case_id: &CaseId,
        role: &Role,
    ) -> Result<Option<ProjectionRootRecord>, PortError> {
        let Some(row) = self
            .fetch_optional_row(graph, ROOT_QUERY, case_id, role, "load projection root")
            .await?
        else {
            return Ok(None);
        };

        let raw = RawProjectionRootRecord {
            latest_summary: row_string(&row, "latest_summary", "projection root")?,
            token_budget_hint: row_i64(&row, "token_budget_hint", "projection root")?,
        };

        Ok(Some(raw.try_into()?))
    }

    async fn load_case_header(
        &self,
        graph: &Graph,
        case_id: &CaseId,
        role: &Role,
    ) -> Result<CaseHeader, PortError> {
        let row = self
            .fetch_optional_row(graph, CASE_HEADER_QUERY, case_id, role, "load case header")
            .await?
            .ok_or_else(|| {
                PortError::InvalidState(format!(
                    "neo4j projection for case `{}` role `{}` is missing a case header",
                    case_id.as_str(),
                    role.as_str()
                ))
            })?;

        RawCaseHeaderRecord {
            case_id: row_string(&row, "case_id", "case header")?,
            title: row_string(&row, "title", "case header")?,
            summary: row_string(&row, "summary", "case header")?,
            status: row_string(&row, "status", "case header")?,
            created_at_millis: row_i64(&row, "created_at", "case header")?,
            created_by: row_string(&row, "created_by", "case header")?,
        }
        .try_into()
    }

    async fn load_plan_header(
        &self,
        graph: &Graph,
        case_id: &CaseId,
        role: &Role,
    ) -> Result<Option<PlanHeader>, PortError> {
        let Some(row) = self
            .fetch_optional_row(graph, PLAN_HEADER_QUERY, case_id, role, "load plan header")
            .await?
        else {
            return Ok(None);
        };

        let raw = RawPlanHeaderRecord {
            plan_id: row_string(&row, "plan_id", "plan header")?,
            revision: row_i64(&row, "revision", "plan header")?,
            status: row_string(&row, "status", "plan header")?,
            work_items_total: row_i64(&row, "work_items_total", "plan header")?,
            work_items_completed: row_i64(&row, "work_items_completed", "plan header")?,
        };

        Ok(Some(raw.try_into()?))
    }

    async fn load_work_items(
        &self,
        graph: &Graph,
        case_id: &CaseId,
        role: &Role,
    ) -> Result<Vec<WorkItem>, PortError> {
        let rows = self
            .fetch_rows(graph, WORK_ITEMS_QUERY, case_id, role, "load work items")
            .await?;

        rows.into_iter()
            .map(|row| {
                RawWorkItemRecord {
                    work_item_id: row_string(&row, "work_item_id", "work item")?,
                    title: row_string(&row, "title", "work item")?,
                    summary: row_string(&row, "summary", "work item")?,
                    role: row_string(&row, "item_role", "work item")?,
                    phase: row_string(&row, "phase", "work item")?,
                    status: row_string(&row, "status", "work item")?,
                    dependency_ids: row_string_vec(&row, "dependency_ids", "work item")?,
                    priority: row_i64(&row, "priority", "work item")?,
                }
                .try_into()
            })
            .collect()
    }

    async fn load_decisions(
        &self,
        graph: &Graph,
        case_id: &CaseId,
        role: &Role,
    ) -> Result<Vec<Decision>, PortError> {
        let rows = self
            .fetch_rows(graph, DECISIONS_QUERY, case_id, role, "load decisions")
            .await?;

        rows.into_iter()
            .map(|row| {
                RawDecisionRecord {
                    decision_id: row_string(&row, "decision_id", "decision")?,
                    title: row_string(&row, "title", "decision")?,
                    rationale: row_string(&row, "rationale", "decision")?,
                    status: row_string(&row, "status", "decision")?,
                    owner: row_string(&row, "owner", "decision")?,
                    decided_at_millis: row_i64(&row, "decided_at", "decision")?,
                }
                .try_into()
            })
            .collect()
    }

    async fn load_decision_relations(
        &self,
        graph: &Graph,
        case_id: &CaseId,
        role: &Role,
    ) -> Result<Vec<DecisionRelation>, PortError> {
        let rows = self
            .fetch_rows(
                graph,
                DECISION_RELATIONS_QUERY,
                case_id,
                role,
                "load decision relations",
            )
            .await?;

        rows.into_iter()
            .map(|row| {
                Ok(DecisionRelation::new(
                    row_string(&row, "source_decision_id", "decision relation")?,
                    row_string(&row, "target_decision_id", "decision relation")?,
                    row_string(&row, "relation_type", "decision relation")?,
                ))
            })
            .collect()
    }

    async fn load_impacts(
        &self,
        graph: &Graph,
        case_id: &CaseId,
        role: &Role,
    ) -> Result<Vec<TaskImpact>, PortError> {
        let rows = self
            .fetch_rows(graph, IMPACTS_QUERY, case_id, role, "load task impacts")
            .await?;

        rows.into_iter()
            .map(|row| {
                Ok(TaskImpact::new(
                    row_string(&row, "decision_id", "task impact")?,
                    row_string(&row, "work_item_id", "task impact")?,
                    row_string(&row, "title", "task impact")?,
                    row_string(&row, "impact_type", "task impact")?,
                ))
            })
            .collect()
    }

    async fn load_milestones(
        &self,
        graph: &Graph,
        case_id: &CaseId,
        role: &Role,
    ) -> Result<Vec<Milestone>, PortError> {
        let rows = self
            .fetch_rows(graph, MILESTONES_QUERY, case_id, role, "load milestones")
            .await?;

        rows.into_iter()
            .map(|row| {
                RawMilestoneRecord {
                    milestone_type: row_string(&row, "milestone_type", "milestone")?,
                    description: row_string(&row, "description", "milestone")?,
                    occurred_at_millis: row_i64(&row, "occurred_at", "milestone")?,
                    actor: row_string(&row, "actor", "milestone")?,
                }
                .try_into()
            })
            .collect()
    }

    async fn fetch_optional_row(
        &self,
        graph: &Graph,
        statement: &str,
        case_id: &CaseId,
        role: &Role,
        operation: &str,
    ) -> Result<Option<Row>, PortError> {
        let mut rows = graph
            .execute(scoped_query(statement, case_id, role))
            .await
            .map_err(|error| {
                PortError::Unavailable(format!(
                    "neo4j {operation} failed for case `{}` role `{}`: {error}",
                    case_id.as_str(),
                    role.as_str()
                ))
            })?;

        rows.next().await.map_err(|error| {
            PortError::Unavailable(format!(
                "neo4j {operation} stream failed for case `{}` role `{}`: {error}",
                case_id.as_str(),
                role.as_str()
            ))
        })
    }

    async fn fetch_rows(
        &self,
        graph: &Graph,
        statement: &str,
        case_id: &CaseId,
        role: &Role,
        operation: &str,
    ) -> Result<Vec<Row>, PortError> {
        let mut rows = graph
            .execute(scoped_query(statement, case_id, role))
            .await
            .map_err(|error| {
                PortError::Unavailable(format!(
                    "neo4j {operation} failed for case `{}` role `{}`: {error}",
                    case_id.as_str(),
                    role.as_str()
                ))
            })?;

        let mut collected = Vec::new();
        while let Some(row) = rows.next().await.map_err(|error| {
            PortError::Unavailable(format!(
                "neo4j {operation} stream failed for case `{}` role `{}`: {error}",
                case_id.as_str(),
                role.as_str()
            ))
        })? {
            collected.push(row);
        }

        Ok(collected)
    }
}

impl ProjectionReader for Neo4jProjectionReader {
    async fn load_pack(
        &self,
        case_id: &CaseId,
        role: &Role,
    ) -> Result<Option<RoleContextPack>, PortError> {
        let graph = self.graph().await?;
        let Some(root) = self.load_root(&graph, case_id, role).await? else {
            return Ok(None);
        };

        let case_header = self.load_case_header(&graph, case_id, role).await?;
        let plan_header = self.load_plan_header(&graph, case_id, role).await?;
        let work_items = self.load_work_items(&graph, case_id, role).await?;
        let decisions = self.load_decisions(&graph, case_id, role).await?;
        let decision_relations = self.load_decision_relations(&graph, case_id, role).await?;
        let impacts = self.load_impacts(&graph, case_id, role).await?;
        let milestones = self.load_milestones(&graph, case_id, role).await?;

        Ok(Some(RoleContextPack::new(
            role.clone(),
            case_header,
            plan_header,
            work_items,
            decisions,
            decision_relations,
            impacts,
            milestones,
            root.latest_summary,
            root.token_budget_hint,
        )))
    }
}

impl Neo4jEndpoint {
    fn parse(graph_uri: String) -> Result<Self, PortError> {
        let graph_uri = graph_uri.trim().to_string();
        if graph_uri.is_empty() {
            return Err(PortError::InvalidState(
                "graph uri cannot be empty".to_string(),
            ));
        }

        let uri = split_uri(&graph_uri, "graph")?;
        if !matches!(
            uri.scheme,
            "neo4j" | "neo4j+s" | "neo4j+ssc" | "bolt" | "bolt+s" | "bolt+ssc"
        ) {
            return Err(PortError::InvalidState(format!(
                "unsupported graph scheme `{}`",
                uri.scheme
            )));
        }

        let authority = parse_authority(uri.authority, "graph")?;
        if uri.query.is_some() {
            return Err(PortError::InvalidState(
                "graph uri query params are not supported yet".to_string(),
            ));
        }

        Ok(Self {
            connection_uri: format!("{}://{}", uri.scheme, authority.host_port),
            user: authority.user.unwrap_or_default(),
            password: authority.password.unwrap_or_default(),
        })
    }
}

struct UriParts<'a> {
    scheme: &'a str,
    authority: &'a str,
    query: Option<&'a str>,
}

#[derive(Debug)]
struct AuthorityParts {
    host_port: String,
    user: Option<String>,
    password: Option<String>,
}

fn split_uri<'a>(raw_uri: &'a str, name: &str) -> Result<UriParts<'a>, PortError> {
    let (scheme, remainder) = raw_uri
        .split_once("://")
        .ok_or_else(|| PortError::InvalidState(format!("{name} uri must include a scheme")))?;
    if scheme.is_empty() {
        return Err(PortError::InvalidState(format!(
            "{name} uri must include a scheme"
        )));
    }

    let (before_query, query) = match remainder.split_once('?') {
        Some((authority_and_path, query)) => (authority_and_path, Some(query)),
        None => (remainder, None),
    };

    let (authority, path) = match before_query.split_once('/') {
        Some((authority, path)) => (authority.trim(), path),
        None => (before_query.trim(), ""),
    };
    if authority.is_empty() {
        return Err(PortError::InvalidState(format!(
            "{name} uri must include a host"
        )));
    }
    if !path.is_empty() {
        return Err(PortError::InvalidState(format!(
            "{name} uri path segments are not supported"
        )));
    }

    Ok(UriParts {
        scheme,
        authority,
        query,
    })
}

fn parse_authority(authority: &str, name: &str) -> Result<AuthorityParts, PortError> {
    let (credentials, host_port) = match authority.rsplit_once('@') {
        Some((credentials, host_port)) => (Some(credentials), host_port),
        None => (None, authority),
    };

    let (user, password) = match credentials {
        Some(credentials) => {
            let (user, password) = credentials.split_once(':').ok_or_else(|| {
                PortError::InvalidState(format!(
                    "{name} uri auth segments must include username and password"
                ))
            })?;
            if user.is_empty() || password.is_empty() {
                return Err(PortError::InvalidState(format!(
                    "{name} uri auth segments must include username and password"
                )));
            }
            (Some(user.to_string()), Some(password.to_string()))
        }
        None => (None, None),
    };

    parse_host_port(host_port, name)?;

    Ok(AuthorityParts {
        host_port: host_port.to_string(),
        user,
        password,
    })
}

fn parse_host_port(authority: &str, name: &str) -> Result<(), PortError> {
    if authority.starts_with('[') {
        let (_, remainder) = authority.split_once(']').ok_or_else(|| {
            PortError::InvalidState(format!("{name} uri contains an invalid IPv6 host"))
        })?;
        if !remainder.is_empty() && !remainder.starts_with(':') {
            return Err(PortError::InvalidState(format!(
                "{name} uri contains an invalid port separator"
            )));
        }
        if let Some(port) = remainder.strip_prefix(':') {
            port.parse::<u16>().map_err(|error| {
                PortError::InvalidState(format!("{name} uri contains an invalid port: {error}"))
            })?;
        }
        return Ok(());
    }

    if let Some((host, port)) = authority.rsplit_once(':') {
        if host.is_empty() {
            return Err(PortError::InvalidState(format!(
                "{name} uri must include a host"
            )));
        }
        if !port.is_empty() {
            port.parse::<u16>().map_err(|error| {
                PortError::InvalidState(format!("{name} uri contains an invalid port: {error}"))
            })?;
        }
    }

    Ok(())
}

fn scoped_query(statement: &str, case_id: &CaseId, role: &Role) -> Query {
    query(statement)
        .param("case_id", case_id.as_str())
        .param("role", role.as_str())
}

fn row_string(row: &Row, key: &str, entity: &str) -> Result<String, PortError> {
    row.get(key).map_err(|error| {
        PortError::InvalidState(format!(
            "neo4j {entity} field `{key}` could not be decoded: {error}"
        ))
    })
}

fn row_i64(row: &Row, key: &str, entity: &str) -> Result<i64, PortError> {
    row.get(key).map_err(|error| {
        PortError::InvalidState(format!(
            "neo4j {entity} field `{key}` could not be decoded: {error}"
        ))
    })
}

fn row_string_vec(row: &Row, key: &str, entity: &str) -> Result<Vec<String>, PortError> {
    row.get(key).map_err(|error| {
        PortError::InvalidState(format!(
            "neo4j {entity} field `{key}` could not be decoded: {error}"
        ))
    })
}

fn parse_u32(value: i64, field: &str) -> Result<u32, PortError> {
    u32::try_from(value).map_err(|_| {
        PortError::InvalidState(format!(
            "neo4j projection field `{field}` must be a non-negative u32"
        ))
    })
}

fn parse_u64(value: i64, field: &str) -> Result<u64, PortError> {
    u64::try_from(value).map_err(|_| {
        PortError::InvalidState(format!(
            "neo4j projection field `{field}` must be a non-negative u64"
        ))
    })
}

fn parse_system_time(millis: i64, field: &str) -> Result<SystemTime, PortError> {
    let millis = u64::try_from(millis).map_err(|_| {
        PortError::InvalidState(format!(
            "neo4j projection field `{field}` must be a non-negative unix timestamp in milliseconds"
        ))
    })?;

    Ok(SystemTime::UNIX_EPOCH + Duration::from_millis(millis))
}

impl TryFrom<RawProjectionRootRecord> for ProjectionRootRecord {
    type Error = PortError;

    fn try_from(value: RawProjectionRootRecord) -> Result<Self, Self::Error> {
        Ok(Self {
            latest_summary: value.latest_summary,
            token_budget_hint: parse_u32(value.token_budget_hint, "token_budget_hint")?,
        })
    }
}

impl TryFrom<RawCaseHeaderRecord> for CaseHeader {
    type Error = PortError;

    fn try_from(value: RawCaseHeaderRecord) -> Result<Self, Self::Error> {
        Ok(Self::new(
            CaseId::new(value.case_id)
                .map_err(|error| PortError::InvalidState(error.to_string()))?,
            value.title,
            value.summary,
            value.status,
            parse_system_time(value.created_at_millis, "case_header.created_at")?,
            value.created_by,
        ))
    }
}

impl TryFrom<RawPlanHeaderRecord> for PlanHeader {
    type Error = PortError;

    fn try_from(value: RawPlanHeaderRecord) -> Result<Self, Self::Error> {
        Ok(Self::new(
            value.plan_id,
            parse_u64(value.revision, "plan_header.revision")?,
            value.status,
            parse_u32(value.work_items_total, "plan_header.work_items_total")?,
            parse_u32(
                value.work_items_completed,
                "plan_header.work_items_completed",
            )?,
        ))
    }
}

impl TryFrom<RawWorkItemRecord> for WorkItem {
    type Error = PortError;

    fn try_from(value: RawWorkItemRecord) -> Result<Self, Self::Error> {
        Ok(Self::new(
            value.work_item_id,
            value.title,
            value.summary,
            value.role,
            value.phase,
            value.status,
            value.dependency_ids,
            parse_u32(value.priority, "work_item.priority")?,
        ))
    }
}

impl TryFrom<RawDecisionRecord> for Decision {
    type Error = PortError;

    fn try_from(value: RawDecisionRecord) -> Result<Self, Self::Error> {
        Ok(Self::new(
            value.decision_id,
            value.title,
            value.rationale,
            value.status,
            value.owner,
            parse_system_time(value.decided_at_millis, "decision.decided_at")?,
        ))
    }
}

impl TryFrom<RawMilestoneRecord> for Milestone {
    type Error = PortError;

    fn try_from(value: RawMilestoneRecord) -> Result<Self, Self::Error> {
        Ok(Self::new(
            value.milestone_type,
            value.description,
            parse_system_time(value.occurred_at_millis, "milestone.occurred_at")?,
            value.actor,
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use rehydration_domain::{CaseId, Decision, Milestone, PlanHeader, WorkItem};

    use super::{
        Neo4jEndpoint, RawCaseHeaderRecord, RawDecisionRecord, RawMilestoneRecord,
        RawPlanHeaderRecord, RawProjectionRootRecord, RawWorkItemRecord, parse_authority,
        parse_host_port, parse_system_time, split_uri,
    };

    #[test]
    fn endpoint_supports_auth_segments() {
        let endpoint = Neo4jEndpoint::parse("neo4j://neo4j:neo@localhost:7687".to_string())
            .expect("uri should parse");

        assert_eq!(endpoint.connection_uri, "neo4j://localhost:7687");
        assert_eq!(endpoint.user, "neo4j");
        assert_eq!(endpoint.password, "neo");
    }

    #[test]
    fn endpoint_rejects_query_params_and_paths() {
        let with_query = Neo4jEndpoint::parse("neo4j://localhost:7687?db=neo4j".to_string())
            .expect_err("query params are not supported yet");
        let with_path = Neo4jEndpoint::parse("neo4j://localhost:7687/graph".to_string())
            .expect_err("paths are not supported");

        assert_eq!(
            with_query,
            rehydration_ports::PortError::InvalidState(
                "graph uri query params are not supported yet".to_string()
            )
        );
        assert_eq!(
            with_path,
            rehydration_ports::PortError::InvalidState(
                "graph uri path segments are not supported".to_string()
            )
        );
    }

    #[test]
    fn parser_rejects_invalid_scheme() {
        let error = Neo4jEndpoint::parse("https://localhost:7687".to_string())
            .expect_err("unsupported schemes must fail");

        assert_eq!(
            error,
            rehydration_ports::PortError::InvalidState(
                "unsupported graph scheme `https`".to_string()
            )
        );
    }

    #[test]
    fn parser_rejects_missing_scheme_and_host() {
        let missing_scheme =
            Neo4jEndpoint::parse("localhost:7687".to_string()).expect_err("scheme is required");
        let missing_host =
            Neo4jEndpoint::parse("neo4j://".to_string()).expect_err("host is required");

        assert_eq!(
            missing_scheme,
            rehydration_ports::PortError::InvalidState(
                "graph uri must include a scheme".to_string()
            )
        );
        assert_eq!(
            missing_host,
            rehydration_ports::PortError::InvalidState("graph uri must include a host".to_string())
        );
    }

    #[test]
    fn parser_rejects_unsupported_authorities() {
        let missing_password = parse_authority("neo4j@localhost:7687", "graph")
            .expect_err("auth must include password");
        let invalid_separator = parse_host_port("[::1]7687", "graph")
            .expect_err("ipv6 port separator must be explicit");
        let invalid_port =
            parse_host_port("localhost:not-a-port", "graph").expect_err("port must be numeric");

        assert_eq!(
            missing_password,
            rehydration_ports::PortError::InvalidState(
                "graph uri auth segments must include username and password".to_string()
            )
        );
        assert_eq!(
            invalid_separator,
            rehydration_ports::PortError::InvalidState(
                "graph uri contains an invalid port separator".to_string()
            )
        );
        assert!(
            invalid_port
                .to_string()
                .starts_with("graph uri contains an invalid port:")
        );
    }

    #[test]
    fn split_uri_supports_ipv6_without_losing_authority() {
        let uri = split_uri("neo4j://[::1]:7687", "graph").expect("uri should parse");
        parse_host_port(uri.authority, "graph").expect("ipv6 authority should be valid");

        assert_eq!(uri.scheme, "neo4j");
        assert_eq!(uri.authority, "[::1]:7687");
        assert!(uri.query.is_none());
    }

    #[test]
    fn root_record_requires_non_negative_token_budget() {
        let error = super::ProjectionRootRecord::try_from(RawProjectionRootRecord {
            latest_summary: "latest".to_string(),
            token_budget_hint: -1,
        })
        .expect_err("negative token budget must fail");

        assert_eq!(
            error,
            rehydration_ports::PortError::InvalidState(
                "neo4j projection field `token_budget_hint` must be a non-negative u32".to_string()
            )
        );
    }

    #[test]
    fn raw_case_header_maps_to_domain() {
        let created_at = 1_728_345_600_000_i64;
        let header = rehydration_domain::CaseHeader::try_from(RawCaseHeaderRecord {
            case_id: "case-123".to_string(),
            title: "Graph-backed case".to_string(),
            summary: "Loaded from neo4j".to_string(),
            status: "ACTIVE".to_string(),
            created_at_millis: created_at,
            created_by: "planner".to_string(),
        })
        .expect("record should map");

        assert_eq!(
            header.case_id(),
            &CaseId::new("case-123").expect("case id is valid")
        );
        assert_eq!(header.title(), "Graph-backed case");
        assert_eq!(
            header.created_at(),
            SystemTime::UNIX_EPOCH + Duration::from_millis(created_at as u64)
        );
    }

    #[test]
    fn raw_plan_header_maps_to_domain() {
        let plan = PlanHeader::try_from(RawPlanHeaderRecord {
            plan_id: "plan-123".to_string(),
            revision: 7,
            status: "ACTIVE".to_string(),
            work_items_total: 10,
            work_items_completed: 4,
        })
        .expect("record should map");

        assert_eq!(plan.plan_id(), "plan-123");
        assert_eq!(plan.revision(), 7);
        assert_eq!(plan.work_items_total(), 10);
    }

    #[test]
    fn raw_work_item_maps_to_domain() {
        let work_item = WorkItem::try_from(RawWorkItemRecord {
            work_item_id: "story-123".to_string(),
            title: "Implement read model".to_string(),
            summary: "Port the first real query".to_string(),
            role: "developer".to_string(),
            phase: "delivery".to_string(),
            status: "in_progress".to_string(),
            dependency_ids: vec!["story-100".to_string()],
            priority: 90,
        })
        .expect("record should map");

        assert_eq!(work_item.work_item_id(), "story-123");
        assert_eq!(work_item.dependency_ids(), &["story-100".to_string()]);
        assert_eq!(work_item.priority(), 90);
    }

    #[test]
    fn raw_decision_and_milestone_map_to_domain() {
        let decision = Decision::try_from(RawDecisionRecord {
            decision_id: "dec-1".to_string(),
            title: "Use Neo4j projection".to_string(),
            rationale: "Supports graph traversal later".to_string(),
            status: "accepted".to_string(),
            owner: "architect".to_string(),
            decided_at_millis: 10,
        })
        .expect("decision should map");
        let milestone = Milestone::try_from(RawMilestoneRecord {
            milestone_type: "phase_transition".to_string(),
            description: "Moved to delivery".to_string(),
            occurred_at_millis: 20,
            actor: "planner".to_string(),
        })
        .expect("milestone should map");

        assert_eq!(decision.decision_id(), "dec-1");
        assert_eq!(decision.owner(), "architect");
        assert_eq!(milestone.milestone_type(), "phase_transition");
        assert_eq!(milestone.actor(), "planner");
    }

    #[test]
    fn timestamps_must_be_non_negative() {
        let error = parse_system_time(-1, "case_header.created_at")
            .expect_err("negative timestamps must fail");

        assert_eq!(
            error,
            rehydration_ports::PortError::InvalidState(
                "neo4j projection field `case_header.created_at` must be a non-negative unix timestamp in milliseconds".to_string()
            )
        );
    }
}
