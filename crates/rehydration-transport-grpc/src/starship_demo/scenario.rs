use std::collections::BTreeMap;
use std::error::Error;

use async_nats::Client;
use rehydration_application::{
    GraphNodeMaterializedData, GraphNodeMaterializedEvent, NodeDetailMaterializedData,
    NodeDetailMaterializedEvent, ProjectionEnvelope, RelatedNodeReference,
};

use crate::agentic_reference::debug_log_value;

pub const MISSION_ROOT_NODE_ID: &str = "node:mission:repair-the-starship";
pub const MISSION_ROOT_NODE_KIND: &str = "mission";
pub const MISSION_ROOT_TITLE: &str = "Repair The Starship";

pub const STEP_ONE_NODE_ID: &str = "node:work_item:stabilize-sensors-and-hull";
pub const STEP_ONE_TITLE: &str = "Stabilize sensors and repair the hull";
pub const STEP_TWO_NODE_ID: &str = "node:work_item:plot-route-and-report-status";
pub const STEP_TWO_TITLE: &str = "Plot route and report ship status";

pub const STARSHIP_STATE_PATH: &str = "state/starship-state.json";
pub const SCAN_COMMAND_PATH: &str = "src/commands/scan.rs";
pub const REPAIR_COMMAND_PATH: &str = "src/commands/repair.rs";
pub const ROUTE_COMMAND_PATH: &str = "src/commands/route.rs";
pub const STATUS_COMMAND_PATH: &str = "src/commands/status.rs";
pub const STARSHIP_TEST_PATH: &str = "tests/starship_cli.rs";
pub const CAPTAINS_LOG_PATH: &str = "captains-log.md";

pub const STEP_ONE_DETAIL: &str = "Phase 1: implement scan and repair commands, then persist the repaired ship state in state/starship-state.json. Stop after those deliverables are written.";
pub const STEP_TWO_DETAIL: &str = "Phase 2: continue from the existing scan, repair, and ship-state artifacts. Implement route and status commands, add tests, and write captains-log.md without rewriting phase 1 files.";

const SUBJECT_PREFIX: &str = "rehydration";
const CONTAINS_RELATION: &str = "contains";
const DEPENDS_ON_RELATION: &str = "depends_on";

type ProjectionMessages = Result<Vec<(String, Vec<u8>)>, Box<dyn Error + Send + Sync>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StarshipScenario {
    run_id: String,
    root_node_id: String,
    root_node_kind: String,
    root_title: String,
    step_one_node_id: String,
    step_two_node_id: String,
}

impl StarshipScenario {
    pub fn reference() -> Self {
        Self {
            run_id: "reference".to_string(),
            root_node_id: MISSION_ROOT_NODE_ID.to_string(),
            root_node_kind: MISSION_ROOT_NODE_KIND.to_string(),
            root_title: MISSION_ROOT_TITLE.to_string(),
            step_one_node_id: STEP_ONE_NODE_ID.to_string(),
            step_two_node_id: STEP_TWO_NODE_ID.to_string(),
        }
    }

    pub fn for_run_id(run_id: impl Into<String>) -> Self {
        let run_id = sanitize_run_id(&run_id.into());
        Self {
            root_node_id: format!("{MISSION_ROOT_NODE_ID}:{run_id}"),
            root_node_kind: MISSION_ROOT_NODE_KIND.to_string(),
            root_title: MISSION_ROOT_TITLE.to_string(),
            step_one_node_id: format!("{STEP_ONE_NODE_ID}:{run_id}"),
            step_two_node_id: format!("{STEP_TWO_NODE_ID}:{run_id}"),
            run_id,
        }
    }

    pub fn run_id(&self) -> &str {
        &self.run_id
    }

    pub fn root_node_id(&self) -> &str {
        &self.root_node_id
    }

    pub fn root_node_kind(&self) -> &str {
        &self.root_node_kind
    }

    pub fn root_title(&self) -> &str {
        &self.root_title
    }

    pub fn step_one_node_id(&self) -> &str {
        &self.step_one_node_id
    }

    pub fn step_two_node_id(&self) -> &str {
        &self.step_two_node_id
    }

    pub async fn publish_initial_projection_events(
        &self,
        client: &Client,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        publish_events(client, self.initial_messages()?).await
    }

    pub async fn publish_resume_projection_events(
        &self,
        client: &Client,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        publish_events(client, self.resume_messages()?).await
    }

    fn initial_messages(&self) -> ProjectionMessages {
        Ok(vec![
            event_payload("graph.node.materialized", self.root_node_event())?,
            event_payload(
                "graph.node.materialized",
                self.step_one_node_event("initial", "IN_PROGRESS"),
            )?,
            event_payload(
                "graph.node.materialized",
                self.step_two_node_event("initial", "PENDING"),
            )?,
            event_payload("node.detail.materialized", self.step_one_detail_event())?,
        ])
    }

    fn resume_messages(&self) -> ProjectionMessages {
        Ok(vec![
            event_payload(
                "graph.node.materialized",
                self.step_one_node_event("resume", "COMPLETED"),
            )?,
            event_payload(
                "graph.node.materialized",
                self.step_two_node_event("resume", "IN_PROGRESS"),
            )?,
            event_payload("node.detail.materialized", self.step_two_detail_event())?,
        ])
    }

    fn root_node_event(&self) -> GraphNodeMaterializedEvent {
        GraphNodeMaterializedEvent {
            envelope: self.envelope("root", &self.root_node_id, &self.root_node_kind),
            data: GraphNodeMaterializedData {
                node_id: self.root_node_id.clone(),
                node_kind: self.root_node_kind.clone(),
                title: self.root_title.clone(),
                summary: "A damaged exploration vessel needs a phased recovery mission."
                    .to_string(),
                status: "ACTIVE".to_string(),
                labels: vec!["agentic-demo".to_string(), "rehydration".to_string()],
                properties: BTreeMap::from([
                    ("theme".to_string(), "starship".to_string()),
                    ("mission".to_string(), "repair".to_string()),
                    ("run_id".to_string(), self.run_id.clone()),
                ]),
                related_nodes: vec![
                    RelatedNodeReference {
                        node_id: self.step_one_node_id.clone(),
                        relation_type: CONTAINS_RELATION.to_string(),
                    },
                    RelatedNodeReference {
                        node_id: self.step_two_node_id.clone(),
                        relation_type: CONTAINS_RELATION.to_string(),
                    },
                    RelatedNodeReference {
                        node_id: self.dependency_node_id(),
                        relation_type: DEPENDS_ON_RELATION.to_string(),
                    },
                ],
            },
        }
    }

    fn step_one_node_event(&self, stage: &str, status: &str) -> GraphNodeMaterializedEvent {
        GraphNodeMaterializedEvent {
            envelope: self.envelope(
                &format!("step-one-{stage}"),
                &self.step_one_node_id,
                "work_item",
            ),
            data: GraphNodeMaterializedData {
                node_id: self.step_one_node_id.clone(),
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
                    ("run_id".to_string(), self.run_id.clone()),
                ]),
                related_nodes: Vec::new(),
            },
        }
    }

    fn step_two_node_event(&self, stage: &str, status: &str) -> GraphNodeMaterializedEvent {
        GraphNodeMaterializedEvent {
            envelope: self.envelope(
                &format!("step-two-{stage}"),
                &self.step_two_node_id,
                "work_item",
            ),
            data: GraphNodeMaterializedData {
                node_id: self.step_two_node_id.clone(),
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
                    ("run_id".to_string(), self.run_id.clone()),
                ]),
                related_nodes: Vec::new(),
            },
        }
    }

    fn step_one_detail_event(&self) -> NodeDetailMaterializedEvent {
        NodeDetailMaterializedEvent {
            envelope: self.envelope("detail-one", &self.step_one_node_id, "node_detail"),
            data: NodeDetailMaterializedData {
                node_id: self.step_one_node_id.clone(),
                detail: STEP_ONE_DETAIL.to_string(),
                content_hash: format!("sha256:starship-phase-1-{}", self.run_id),
                revision: 1,
            },
        }
    }

    fn step_two_detail_event(&self) -> NodeDetailMaterializedEvent {
        NodeDetailMaterializedEvent {
            envelope: self.envelope("detail-two", &self.step_two_node_id, "node_detail"),
            data: NodeDetailMaterializedData {
                node_id: self.step_two_node_id.clone(),
                detail: STEP_TWO_DETAIL.to_string(),
                content_hash: format!("sha256:starship-phase-2-{}", self.run_id),
                revision: 2,
            },
        }
    }

    fn envelope(
        &self,
        event_kind: &str,
        aggregate_id: &str,
        aggregate_type: &str,
    ) -> ProjectionEnvelope {
        ProjectionEnvelope {
            event_id: format!("evt-starship-{}-{event_kind}", self.run_id),
            correlation_id: format!("corr-starship-{}", self.run_id),
            causation_id: format!("cause-starship-{}", self.run_id),
            occurred_at: "2026-03-12T09:00:00Z".to_string(),
            aggregate_id: aggregate_id.to_string(),
            aggregate_type: aggregate_type.to_string(),
            schema_version: "v1alpha1".to_string(),
        }
    }

    fn dependency_node_id(&self) -> String {
        if self.run_id == "reference" {
            "node:dependency:navigation-core".to_string()
        } else {
            format!("node:dependency:navigation-core:{}", self.run_id)
        }
    }
}

pub async fn publish_initial_projection_events(
    client: &Client,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    StarshipScenario::reference()
        .publish_initial_projection_events(client)
        .await
}

pub async fn publish_resume_projection_events(
    client: &Client,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    StarshipScenario::reference()
        .publish_resume_projection_events(client)
        .await
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

fn event_payload<T: serde::Serialize>(
    suffix: &str,
    event: T,
) -> Result<(String, Vec<u8>), Box<dyn Error + Send + Sync>> {
    Ok((subject(suffix), serde_json::to_vec(&event)?))
}

fn subject(suffix: &str) -> String {
    format!("{SUBJECT_PREFIX}.{suffix}")
}

fn sanitize_run_id(run_id: &str) -> String {
    let sanitized = run_id
        .chars()
        .map(|character| match character {
            'a'..='z' | 'A'..='Z' | '0'..='9' => character.to_ascii_lowercase(),
            '-' | '_' | ':' => '-',
            _ => '-',
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    if sanitized.is_empty() {
        "starship-demo".to_string()
    } else {
        sanitized
    }
}

#[cfg(test)]
mod tests {
    use super::StarshipScenario;

    #[test]
    fn dynamic_scenario_generates_distinct_ids() {
        let scenario = StarshipScenario::for_run_id("demo-42");

        assert!(scenario.root_node_id().ends_with(":demo-42"));
        assert!(scenario.step_one_node_id().ends_with(":demo-42"));
        assert!(scenario.step_two_node_id().ends_with(":demo-42"));
    }

    #[test]
    fn reference_scenario_preserves_stable_ids() {
        let scenario = StarshipScenario::reference();

        assert_eq!(scenario.root_node_id(), "node:mission:repair-the-starship");
        assert_eq!(
            scenario.step_two_node_id(),
            "node:work_item:plot-route-and-report-status"
        );
    }
}
