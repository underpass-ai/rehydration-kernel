use std::collections::BTreeMap;
use std::error::Error;

use async_nats::Client;
use rehydration_application::{
    GraphNodeMaterializedData, GraphNodeMaterializedEvent, NodeDetailMaterializedData,
    NodeDetailMaterializedEvent, ProjectionEnvelope, RelatedNodeReference,
};

use crate::agentic_support::agentic_debug::debug_log_value;
use crate::agentic_support::seed_data::{
    DECISION_DETAIL, DECISION_DETAIL_HASH, DECISION_DETAIL_REVISION, DECISION_ID, DECISION_LABEL,
    DECISION_STATUS, DECISION_SUMMARY, DECISION_TITLE, HAS_TASK_RELATION, RECORDS_RELATION,
    ROOT_CREATED_BY, ROOT_DETAIL, ROOT_DETAIL_HASH, ROOT_DETAIL_REVISION, ROOT_LABEL, ROOT_NODE_ID,
    ROOT_PLAN_ID, ROOT_STATUS, ROOT_SUMMARY, ROOT_TITLE, TASK_ID, TASK_LABEL, TASK_PRIORITY,
    TASK_ROLE, TASK_STATUS, TASK_SUMMARY, TASK_TITLE,
};

pub(crate) const TASK_DETAIL: &str =
    "The task wires the compatibility shell and v1alpha1 API into the same kernel surface.";
pub(crate) const SUBJECT_PREFIX: &str = "rehydration";

type ProjectionMessagesResult = Result<Vec<(String, Vec<u8>)>, Box<dyn Error + Send + Sync>>;

pub(crate) async fn publish_kernel_e2e_projection_events(
    client: &Client,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    for (subject, payload) in projection_messages()? {
        debug_log_value("publishing kernel e2e subject", &subject);
        client.publish(subject, payload.into()).await?;
    }
    client.flush().await?;
    Ok(())
}

fn projection_messages() -> ProjectionMessagesResult {
    Ok(vec![
        (
            subject("graph.node.materialized"),
            serde_json::to_vec(&root_node_event())?,
        ),
        (
            subject("graph.node.materialized"),
            serde_json::to_vec(&decision_node_event())?,
        ),
        (
            subject("graph.node.materialized"),
            serde_json::to_vec(&task_node_event())?,
        ),
        (
            subject("node.detail.materialized"),
            serde_json::to_vec(&root_detail_event())?,
        ),
        (
            subject("node.detail.materialized"),
            serde_json::to_vec(&decision_detail_event())?,
        ),
        (
            subject("node.detail.materialized"),
            serde_json::to_vec(&task_detail_event())?,
        ),
    ])
}

fn subject(suffix: &str) -> String {
    format!("{SUBJECT_PREFIX}.{suffix}")
}

fn root_node_event() -> GraphNodeMaterializedEvent {
    GraphNodeMaterializedEvent {
        envelope: base_envelope("evt-kernel-root-1", ROOT_NODE_ID, "node"),
        data: GraphNodeMaterializedData {
            node_id: ROOT_NODE_ID.to_string(),
            node_kind: ROOT_LABEL.to_string(),
            title: ROOT_TITLE.to_string(),
            summary: ROOT_SUMMARY.to_string(),
            status: ROOT_STATUS.to_string(),
            labels: vec![ROOT_LABEL.to_string()],
            properties: BTreeMap::from([
                ("created_by".to_string(), ROOT_CREATED_BY.to_string()),
                ("plan_id".to_string(), ROOT_PLAN_ID.to_string()),
            ]),
            related_nodes: vec![
                RelatedNodeReference {
                    node_id: DECISION_ID.to_string(),
                    relation_type: RECORDS_RELATION.to_string(),
                },
                RelatedNodeReference {
                    node_id: TASK_ID.to_string(),
                    relation_type: HAS_TASK_RELATION.to_string(),
                },
            ],
        },
    }
}

fn decision_node_event() -> GraphNodeMaterializedEvent {
    GraphNodeMaterializedEvent {
        envelope: base_envelope("evt-kernel-decision-1", DECISION_ID, "node"),
        data: GraphNodeMaterializedData {
            node_id: DECISION_ID.to_string(),
            node_kind: DECISION_LABEL.to_string(),
            title: DECISION_TITLE.to_string(),
            summary: DECISION_SUMMARY.to_string(),
            status: DECISION_STATUS.to_string(),
            labels: vec![DECISION_LABEL.to_string()],
            properties: BTreeMap::new(),
            related_nodes: Vec::new(),
        },
    }
}

fn task_node_event() -> GraphNodeMaterializedEvent {
    GraphNodeMaterializedEvent {
        envelope: base_envelope("evt-kernel-task-1", TASK_ID, "node"),
        data: GraphNodeMaterializedData {
            node_id: TASK_ID.to_string(),
            node_kind: TASK_LABEL.to_string(),
            title: TASK_TITLE.to_string(),
            summary: TASK_SUMMARY.to_string(),
            status: TASK_STATUS.to_string(),
            labels: vec![TASK_LABEL.to_string()],
            properties: BTreeMap::from([
                ("role".to_string(), TASK_ROLE.to_string()),
                ("priority".to_string(), TASK_PRIORITY.to_string()),
            ]),
            related_nodes: Vec::new(),
        },
    }
}

fn root_detail_event() -> NodeDetailMaterializedEvent {
    NodeDetailMaterializedEvent {
        envelope: base_envelope("evt-kernel-root-detail-1", ROOT_NODE_ID, "node_detail"),
        data: NodeDetailMaterializedData {
            node_id: ROOT_NODE_ID.to_string(),
            detail: ROOT_DETAIL.to_string(),
            content_hash: ROOT_DETAIL_HASH.to_string(),
            revision: ROOT_DETAIL_REVISION,
        },
    }
}

fn decision_detail_event() -> NodeDetailMaterializedEvent {
    NodeDetailMaterializedEvent {
        envelope: base_envelope("evt-kernel-decision-detail-1", DECISION_ID, "node_detail"),
        data: NodeDetailMaterializedData {
            node_id: DECISION_ID.to_string(),
            detail: DECISION_DETAIL.to_string(),
            content_hash: DECISION_DETAIL_HASH.to_string(),
            revision: DECISION_DETAIL_REVISION,
        },
    }
}

fn task_detail_event() -> NodeDetailMaterializedEvent {
    NodeDetailMaterializedEvent {
        envelope: base_envelope("evt-kernel-task-detail-1", TASK_ID, "node_detail"),
        data: NodeDetailMaterializedData {
            node_id: TASK_ID.to_string(),
            detail: TASK_DETAIL.to_string(),
            content_hash: "hash-task-detail".to_string(),
            revision: 1,
        },
    }
}

fn base_envelope(event_id: &str, aggregate_id: &str, aggregate_type: &str) -> ProjectionEnvelope {
    ProjectionEnvelope {
        event_id: event_id.to_string(),
        correlation_id: "corr-kernel-e2e".to_string(),
        causation_id: "cause-kernel-e2e".to_string(),
        occurred_at: "2026-03-18T00:00:00Z".to_string(),
        aggregate_id: aggregate_id.to_string(),
        aggregate_type: aggregate_type.to_string(),
        schema_version: "v1alpha1".to_string(),
    }
}
