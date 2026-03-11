use std::collections::BTreeSet;
use std::io;
use std::str::FromStr;

use rehydration_proto::v1alpha1::{
    BundleRenderFormat, GetContextRequest, GetGraphRelationshipsRequest, GraphNode, Phase,
    context_admin_service_client::ContextAdminServiceClient,
    context_query_service_client::ContextQueryServiceClient,
};
use serde_json::json;
use tonic::transport::Channel;

use crate::agentic_support::agentic_debug::debug_log_value;
use crate::agentic_support::runtime_workspace::{AgentRuntime, RuntimeResult, ToolDescriptor};
use crate::agentic_support::starship_seed_data::{
    CAPTAINS_LOG_PATH, MISSION_ROOT_TITLE, STARSHIP_STATE_PATH, STEP_TWO_TITLE,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StarshipMissionRequest {
    pub(crate) root_node_id: String,
    pub(crate) root_node_kind: String,
    pub(crate) role: String,
    pub(crate) phase: Phase,
    pub(crate) requested_scopes: Vec<String>,
    pub(crate) token_budget: u32,
}

impl StarshipMissionRequest {
    pub(crate) fn reference_defaults(root_node_id: &str, root_node_kind: &str) -> Self {
        Self {
            root_node_id: root_node_id.to_string(),
            root_node_kind: root_node_kind.to_string(),
            role: "implementer".to_string(),
            phase: Phase::Build,
            requested_scopes: vec!["implementation".to_string(), "continuation".to_string()],
            token_budget: 1800,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StarshipMissionExecution {
    pub(crate) selected_step_node_id: String,
    pub(crate) written_paths: Vec<String>,
    pub(crate) listed_files: Vec<String>,
    pub(crate) captains_log: Option<String>,
}

pub(crate) struct StarshipMissionAgent<R> {
    query_client: ContextQueryServiceClient<Channel>,
    admin_client: ContextAdminServiceClient<Channel>,
    runtime: R,
}

impl<R> StarshipMissionAgent<R>
where
    R: AgentRuntime,
{
    pub(crate) fn new(
        query_client: ContextQueryServiceClient<Channel>,
        admin_client: ContextAdminServiceClient<Channel>,
        runtime: R,
    ) -> Self {
        Self {
            query_client,
            admin_client,
            runtime,
        }
    }

    pub(crate) async fn current_step(
        &mut self,
        request: &StarshipMissionRequest,
    ) -> RuntimeResult<GraphNode> {
        let response = self
            .admin_client
            .get_graph_relationships(GetGraphRelationshipsRequest {
                node_id: request.root_node_id.clone(),
                node_kind: request.root_node_kind.clone(),
                depth: 1,
                include_reverse_edges: false,
            })
            .await?
            .into_inner();

        let mut work_items = response
            .neighbors
            .into_iter()
            .filter(|node| node.node_kind == "work_item")
            .collect::<Vec<_>>();
        work_items.sort_by_key(step_order_key);

        work_items
            .iter()
            .find(|node| node.status == "IN_PROGRESS")
            .cloned()
            .or_else(|| {
                work_items
                    .into_iter()
                    .find(|node| node.status != "COMPLETED")
            })
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotFound, "no resumable starship step found").into()
            })
    }

    pub(crate) async fn execute_next_step(
        &mut self,
        request: StarshipMissionRequest,
    ) -> RuntimeResult<StarshipMissionExecution> {
        let tools = self.runtime.list_tools().await?;
        ensure_runtime_supports_mission_workflow(&tools)?;

        let step = self.current_step(&request).await?;
        debug_log_value("starship current step", &step.node_id);
        let context = self
            .query_client
            .get_context(GetContextRequest {
                root_node_id: request.root_node_id,
                role: request.role,
                phase: request.phase as i32,
                work_item_id: step.node_id.clone(),
                token_budget: request.token_budget,
                requested_scopes: request.requested_scopes,
                render_format: BundleRenderFormat::LlmPrompt as i32,
                include_debug_sections: false,
            })
            .await?
            .into_inner();

        let bundle = context
            .bundle
            .ok_or_else(|| io::Error::other("missing bundle in starship mission context"))?;
        let role_bundle = bundle
            .bundles
            .first()
            .ok_or_else(|| io::Error::other("missing role bundle in starship mission context"))?;
        let step_detail = role_bundle
            .node_details
            .iter()
            .find(|detail| detail.node_id == step.node_id)
            .ok_or_else(|| io::Error::other("missing starship step detail"))?;
        let rendered = context
            .rendered
            .ok_or_else(|| io::Error::other("missing rendered context"))?;

        let listed_files = self
            .runtime
            .invoke("fs.list", json!({ "path": "." }), false)
            .await?
            .output
            .lines()
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .map(ToString::to_string)
            .collect::<BTreeSet<_>>();
        debug_log_value("starship listed files", listed_files.len());

        let previous_state = if listed_files.contains(STARSHIP_STATE_PATH) {
            Some(
                self.runtime
                    .invoke("fs.read", json!({ "path": STARSHIP_STATE_PATH }), false)
                    .await?
                    .output,
            )
        } else {
            None
        };

        let deliverables = step
            .properties
            .get("deliverables")
            .map(|value| {
                value
                    .split(',')
                    .map(str::trim)
                    .filter(|path| !path.is_empty())
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            })
            .ok_or_else(|| io::Error::other("missing deliverables in step properties"))?;

        let mut written_paths = Vec::new();
        for deliverable in deliverables {
            if listed_files.contains(&deliverable) {
                continue;
            }

            self.runtime
                .invoke(
                    "fs.write",
                    json!({
                        "path": deliverable,
                        "content": deliverable_content(
                            &deliverable,
                            &step,
                            &step_detail.detail,
                            &rendered.content,
                            previous_state.as_deref(),
                        ),
                    }),
                    true,
                )
                .await?;
            written_paths.push(deliverable);
        }

        let captains_log = if written_paths.iter().any(|path| path == CAPTAINS_LOG_PATH) {
            Some(
                self.runtime
                    .invoke("fs.read", json!({ "path": CAPTAINS_LOG_PATH }), false)
                    .await?
                    .output,
            )
        } else {
            None
        };

        Ok(StarshipMissionExecution {
            selected_step_node_id: step.node_id,
            written_paths,
            listed_files: listed_files.into_iter().collect(),
            captains_log,
        })
    }
}

fn ensure_runtime_supports_mission_workflow(tools: &[ToolDescriptor]) -> RuntimeResult<()> {
    for required_tool in ["fs.write", "fs.read", "fs.list"] {
        if !tools.iter().any(|tool| tool.name == required_tool) {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!("runtime is missing required tool `{required_tool}`"),
            )
            .into());
        }
    }

    Ok(())
}

fn step_order_key(node: &GraphNode) -> u32 {
    node.properties
        .get("sequence")
        .and_then(|value| u32::from_str(value).ok())
        .unwrap_or(u32::MAX)
}

fn deliverable_content(
    path: &str,
    step: &GraphNode,
    step_detail: &str,
    rendered_context: &str,
    previous_state: Option<&str>,
) -> String {
    match path {
        "src/commands/scan.rs" => format!(
            "pub fn scan() -> &'static str {{\n    \"sensors online: hull breach localized\"\n}}\n\n// Context: {}\n",
            step_detail
        ),
        "src/commands/repair.rs" => format!(
            "pub fn repair(system: &str) -> String {{\n    format!(\"repairing {{}} with drone swarm\", system)\n}}\n\n// Step: {}\n",
            step.title
        ),
        "state/starship-state.json" => {
            "{\"hull\":\"stabilized\",\"sensors\":\"online\",\"engine\":\"standby\"}\n".to_string()
        }
        "src/commands/route.rs" => format!(
            "pub fn route(destination: &str) -> String {{\n    format!(\"route plotted to {{}} via ion corridor\", destination)\n}}\n\n// Resume detail: {}\n",
            step_detail
        ),
        "src/commands/status.rs" => format!(
            "pub fn status() -> &'static str {{\n    \"ship status: ready for departure\"\n}}\n\n// Rendered context excerpt: {}\n",
            rendered_context
                .lines()
                .next()
                .unwrap_or("context unavailable")
        ),
        "tests/starship_cli.rs" => {
            "assert!(scan().contains(\"hull breach\"));\nassert!(status().contains(\"ready\"));\n"
                .to_string()
        }
        "captains-log.md" => format!(
            "# Captain's Log\n\nMission: {MISSION_ROOT_TITLE}\nCurrent step: {STEP_TWO_TITLE}\n\nRecovered state:\n{}\n\nResume brief:\n{}\n",
            previous_state.unwrap_or("state unavailable"),
            step_detail
        ),
        other => format!(
            "# Generated deliverable\n\nPath: {other}\nStep: {}\nContext:\n{}\n",
            step.title, rendered_context
        ),
    }
}
