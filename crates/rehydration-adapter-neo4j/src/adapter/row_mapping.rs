use std::collections::BTreeMap;
use std::time::{Duration, SystemTime};

use neo4rs::Row;
use rehydration_domain::{
    CaseHeader, CaseId, Decision, Milestone, PlanHeader, TaskImpact, WorkItem,
};
use rehydration_ports::{NodeProjection, PortError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProjectionRootRecord {
    pub(crate) latest_summary: String,
    pub(crate) token_budget_hint: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RawProjectionRootRecord {
    pub(crate) latest_summary: String,
    pub(crate) token_budget_hint: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RawCaseHeaderRecord {
    pub(crate) case_id: String,
    pub(crate) title: String,
    pub(crate) summary: String,
    pub(crate) status: String,
    pub(crate) created_at_millis: i64,
    pub(crate) created_by: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RawPlanHeaderRecord {
    pub(crate) plan_id: String,
    pub(crate) revision: i64,
    pub(crate) status: String,
    pub(crate) work_items_total: i64,
    pub(crate) work_items_completed: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RawWorkItemRecord {
    pub(crate) work_item_id: String,
    pub(crate) title: String,
    pub(crate) summary: String,
    pub(crate) role: String,
    pub(crate) phase: String,
    pub(crate) status: String,
    pub(crate) dependency_ids: Vec<String>,
    pub(crate) priority: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RawDecisionRecord {
    pub(crate) decision_id: String,
    pub(crate) title: String,
    pub(crate) rationale: String,
    pub(crate) status: String,
    pub(crate) owner: String,
    pub(crate) decided_at_millis: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RawMilestoneRecord {
    pub(crate) milestone_type: String,
    pub(crate) description: String,
    pub(crate) occurred_at_millis: i64,
    pub(crate) actor: String,
}

pub(crate) fn serialize_properties(
    properties: &BTreeMap<String, String>,
) -> Result<String, PortError> {
    serde_json::to_string(properties).map_err(|error| {
        PortError::InvalidState(format!(
            "neo4j node projection properties could not be serialized: {error}"
        ))
    })
}

pub(crate) fn deserialize_properties(
    payload: &str,
    entity: &str,
) -> Result<BTreeMap<String, String>, PortError> {
    serde_json::from_str(payload).map_err(|error| {
        PortError::InvalidState(format!(
            "neo4j {entity} properties_json could not be decoded: {error}"
        ))
    })
}

pub(crate) fn row_string(row: &Row, key: &str, entity: &str) -> Result<String, PortError> {
    row.get(key).map_err(|error| {
        PortError::InvalidState(format!(
            "neo4j {entity} field `{key}` could not be decoded: {error}"
        ))
    })
}

pub(crate) fn row_i64(row: &Row, key: &str, entity: &str) -> Result<i64, PortError> {
    row.get(key).map_err(|error| {
        PortError::InvalidState(format!(
            "neo4j {entity} field `{key}` could not be decoded: {error}"
        ))
    })
}

pub(crate) fn row_string_vec(row: &Row, key: &str, entity: &str) -> Result<Vec<String>, PortError> {
    row.get(key).map_err(|error| {
        PortError::InvalidState(format!(
            "neo4j {entity} field `{key}` could not be decoded: {error}"
        ))
    })
}

pub(crate) fn node_projection_from_row(
    row: &Row,
    prefix: &str,
    entity: &str,
) -> Result<NodeProjection, PortError> {
    Ok(NodeProjection {
        node_id: row_string(row, &format!("{prefix}node_id"), entity)?,
        node_kind: row_string(row, &format!("{prefix}node_kind"), entity)?,
        title: row_string(row, &format!("{prefix}title"), entity)?,
        summary: row_string(row, &format!("{prefix}summary"), entity)?,
        status: row_string(row, &format!("{prefix}status"), entity)?,
        labels: row_string_vec(row, &format!("{prefix}node_labels"), entity)?,
        properties: deserialize_properties(
            &row_string(row, &format!("{prefix}properties_json"), entity)?,
            entity,
        )?,
    })
}

pub(crate) fn parse_u32(value: i64, field: &str) -> Result<u32, PortError> {
    u32::try_from(value).map_err(|_| {
        PortError::InvalidState(format!(
            "neo4j projection field `{field}` must be a non-negative u32"
        ))
    })
}

pub(crate) fn parse_u64(value: i64, field: &str) -> Result<u64, PortError> {
    u64::try_from(value).map_err(|_| {
        PortError::InvalidState(format!(
            "neo4j projection field `{field}` must be a non-negative u64"
        ))
    })
}

pub(crate) fn parse_system_time(millis: i64, field: &str) -> Result<SystemTime, PortError> {
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

pub(crate) fn task_impact_from_row(row: &Row) -> Result<TaskImpact, PortError> {
    Ok(TaskImpact::new(
        row_string(row, "decision_id", "task impact")?,
        row_string(row, "work_item_id", "task impact")?,
        row_string(row, "title", "task impact")?,
        row_string(row, "impact_type", "task impact")?,
    ))
}
