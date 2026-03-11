use std::collections::BTreeMap;
use std::error::Error;

use async_nats::Client;
use rehydration_application::{
    GraphNodeMaterializedData, GraphNodeMaterializedEvent, NodeDetailMaterializedData,
    NodeDetailMaterializedEvent, ProjectionEnvelope, RelatedNodeReference,
};

use crate::agentic_support::agentic_debug::debug_log_value;

pub(crate) const MISSION_ROOT_NODE_ID: &str = "node:mission:repair-the-starship";
pub(crate) const MISSION_ROOT_NODE_KIND: &str = "mission";
pub(crate) const MISSION_ROOT_TITLE: &str = "Repair The Starship";

pub(crate) const STEP_ONE_NODE_ID: &str = "node:work_item:stabilize-sensors-and-hull";
pub(crate) const STEP_ONE_TITLE: &str = "Stabilize sensors and repair the hull";
pub(crate) const STEP_TWO_NODE_ID: &str = "node:work_item:plot-route-and-report-status";
pub(crate) const STEP_TWO_TITLE: &str = "Plot route and report ship status";

pub(crate) const STARSHIP_STATE_PATH: &str = "state/starship-state.json";
pub(crate) const SCAN_COMMAND_PATH: &str = "src/commands/scan.rs";
pub(crate) const REPAIR_COMMAND_PATH: &str = "src/commands/repair.rs";
pub(crate) const ROUTE_COMMAND_PATH: &str = "src/commands/route.rs";
pub(crate) const STATUS_COMMAND_PATH: &str = "src/commands/status.rs";
pub(crate) const STARSHIP_TEST_PATH: &str = "tests/starship_cli.rs";
pub(crate) const CAPTAINS_LOG_PATH: &str = "captains-log.md";

pub(crate) const STEP_ONE_DETAIL: &str = "Phase 1: implement scan and repair commands, then persist the repaired ship state in state/starship-state.json. Stop after those deliverables are written.";
pub(crate) const STEP_TWO_DETAIL: &str = "Phase 2: continue from the existing scan, repair, and ship-state artifacts. Implement route and status commands, add tests, and write captains-log.md without rewriting phase 1 files.";

const SUBJECT_PREFIX: &str = "rehydration";
const CONTAINS_RELATION: &str = "contains";
const DEPENDS_ON_RELATION: &str = "depends_on";
type ProjectionMessages = Result<Vec<(String, Vec<u8>)>, Box<dyn Error + Send + Sync>>;

pub(crate) async fn publish_initial_projection_events(
    client: &Client,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    publish_events(client, initial_messages()?).await
}

pub(crate) async fn publish_resume_projection_events(
    client: &Client,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    publish_events(client, resume_messages()?).await
}

async fn publish_events(
    client: &Client,
    messages: Vec<(String, Vec<u8>)>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    for (subject, payload) in messages {
        debug_log_value("publishing starship subject", &subject);
        client.publish(subject, payload.into()).await?;
    }
    client.flush().await?;
    Ok(())
}

fn initial_messages() -> ProjectionMessages {
    Ok(vec![
        event_payload("graph.node.materialized", root_node_event())?,
        event_payload(
            "graph.node.materialized",
            step_one_node_event("evt-starship-step-1-initial", "IN_PROGRESS"),
        )?,
        event_payload(
            "graph.node.materialized",
            step_two_node_event("evt-starship-step-2-initial", "PENDING"),
        )?,
        event_payload("node.detail.materialized", step_one_detail_event())?,
    ])
}

fn resume_messages() -> ProjectionMessages {
    Ok(vec![
        event_payload(
            "graph.node.materialized",
            step_one_node_event("evt-starship-step-1-resume", "COMPLETED"),
        )?,
        event_payload(
            "graph.node.materialized",
            step_two_node_event("evt-starship-step-2-resume", "IN_PROGRESS"),
        )?,
        event_payload("node.detail.materialized", step_two_detail_event())?,
    ])
}

fn event_payload<T: serde::Serialize>(
    suffix: &str,
    event: T,
) -> Result<(String, Vec<u8>), Box<dyn Error + Send + Sync>> {
    Ok((subject(suffix), serde_json::to_vec(&event)?))
}

fn subject(suffix: &str) -> String {
    format!("{SUBJECT_PREFIX}.{suffix}")
}

fn root_node_event() -> GraphNodeMaterializedEvent {
    GraphNodeMaterializedEvent {
        envelope: envelope("evt-starship-root", MISSION_ROOT_NODE_ID, "mission"),
        data: GraphNodeMaterializedData {
            node_id: MISSION_ROOT_NODE_ID.to_string(),
            node_kind: MISSION_ROOT_NODE_KIND.to_string(),
            title: MISSION_ROOT_TITLE.to_string(),
            summary: "A damaged exploration vessel needs a phased recovery mission.".to_string(),
            status: "ACTIVE".to_string(),
            labels: vec!["agentic-demo".to_string(), "rehydration".to_string()],
            properties: BTreeMap::from([
                ("theme".to_string(), "starship".to_string()),
                ("mission".to_string(), "repair".to_string()),
            ]),
            related_nodes: vec![
                RelatedNodeReference {
                    node_id: STEP_ONE_NODE_ID.to_string(),
                    relation_type: CONTAINS_RELATION.to_string(),
                },
                RelatedNodeReference {
                    node_id: STEP_TWO_NODE_ID.to_string(),
                    relation_type: CONTAINS_RELATION.to_string(),
                },
                RelatedNodeReference {
                    node_id: "node:dependency:navigation-core".to_string(),
                    relation_type: DEPENDS_ON_RELATION.to_string(),
                },
            ],
        },
    }
}

fn step_one_node_event(event_id: &str, status: &str) -> GraphNodeMaterializedEvent {
    GraphNodeMaterializedEvent {
        envelope: envelope(event_id, STEP_ONE_NODE_ID, "work_item"),
        data: GraphNodeMaterializedData {
            node_id: STEP_ONE_NODE_ID.to_string(),
            node_kind: "work_item".to_string(),
            title: STEP_ONE_TITLE.to_string(),
            summary: "Bring the ship back to a stable operational state.".to_string(),
            status: status.to_string(),
            labels: vec!["phase-1".to_string()],
            properties: BTreeMap::from([
                ("sequence".to_string(), "1".to_string()),
                (
                    "deliverables".to_string(),
                    [SCAN_COMMAND_PATH, REPAIR_COMMAND_PATH, STARSHIP_STATE_PATH].join(","),
                ),
            ]),
            related_nodes: Vec::new(),
        },
    }
}

fn step_two_node_event(event_id: &str, status: &str) -> GraphNodeMaterializedEvent {
    GraphNodeMaterializedEvent {
        envelope: envelope(event_id, STEP_TWO_NODE_ID, "work_item"),
        data: GraphNodeMaterializedData {
            node_id: STEP_TWO_NODE_ID.to_string(),
            node_kind: "work_item".to_string(),
            title: STEP_TWO_TITLE.to_string(),
            summary: "Resume from the stabilized ship and finish the mission.".to_string(),
            status: status.to_string(),
            labels: vec!["phase-2".to_string()],
            properties: BTreeMap::from([
                ("sequence".to_string(), "2".to_string()),
                (
                    "deliverables".to_string(),
                    [
                        ROUTE_COMMAND_PATH,
                        STATUS_COMMAND_PATH,
                        STARSHIP_TEST_PATH,
                        CAPTAINS_LOG_PATH,
                    ]
                    .join(","),
                ),
            ]),
            related_nodes: Vec::new(),
        },
    }
}

fn step_one_detail_event() -> NodeDetailMaterializedEvent {
    NodeDetailMaterializedEvent {
        envelope: envelope("evt-starship-detail-1", STEP_ONE_NODE_ID, "node_detail"),
        data: NodeDetailMaterializedData {
            node_id: STEP_ONE_NODE_ID.to_string(),
            detail: STEP_ONE_DETAIL.to_string(),
            content_hash: "sha256:starship-phase-1".to_string(),
            revision: 1,
        },
    }
}

fn step_two_detail_event() -> NodeDetailMaterializedEvent {
    NodeDetailMaterializedEvent {
        envelope: envelope("evt-starship-detail-2", STEP_TWO_NODE_ID, "node_detail"),
        data: NodeDetailMaterializedData {
            node_id: STEP_TWO_NODE_ID.to_string(),
            detail: STEP_TWO_DETAIL.to_string(),
            content_hash: "sha256:starship-phase-2".to_string(),
            revision: 2,
        },
    }
}

fn envelope(event_id: &str, aggregate_id: &str, aggregate_type: &str) -> ProjectionEnvelope {
    ProjectionEnvelope {
        event_id: event_id.to_string(),
        correlation_id: "corr-starship-agentic".to_string(),
        causation_id: "cause-starship-agentic".to_string(),
        occurred_at: "2026-03-11T20:00:00Z".to_string(),
        aggregate_id: aggregate_id.to_string(),
        aggregate_type: aggregate_type.to_string(),
        schema_version: "v1alpha1".to_string(),
    }
}
