use std::collections::BTreeSet;
use std::io;

use rehydration_proto::v1alpha1::{
    GetContextRequest, GetGraphRelationshipsRequest, GraphNode,
    context_admin_service_client::ContextAdminServiceClient,
    context_query_service_client::ContextQueryServiceClient,
};
use serde::Deserialize;
use serde_json::json;
use tonic::transport::Channel;

use crate::agentic_reference::{AgentRuntime, RuntimeResult};
use crate::agentic_reference::{debug_log, debug_log_value};
use crate::starship_demo::{CAPTAINS_LOG_PATH, LlmPlanner};

#[derive(Debug, Clone)]
pub struct LlmStarshipMissionRequest {
    pub root_node_id: String,
    pub root_node_kind: String,
    pub role: String,
}

impl LlmStarshipMissionRequest {
    pub fn reference_defaults(root_node_id: &str, root_node_kind: &str) -> Self {
        Self {
            root_node_id: root_node_id.to_string(),
            root_node_kind: root_node_kind.to_string(),
            role: "implementer".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmStarshipMissionExecution {
    pub selected_step_node_id: String,
    pub written_paths: Vec<String>,
    pub captains_log: Option<String>,
}

pub struct LlmStarshipMissionAgent<R> {
    query_client: ContextQueryServiceClient<Channel>,
    admin_client: ContextAdminServiceClient<Channel>,
    runtime: R,
    llm: LlmPlanner,
}

impl<R> LlmStarshipMissionAgent<R>
where
    R: AgentRuntime,
{
    pub fn new(
        query_client: ContextQueryServiceClient<Channel>,
        admin_client: ContextAdminServiceClient<Channel>,
        runtime: R,
        llm: LlmPlanner,
    ) -> Self {
        Self {
            query_client,
            admin_client,
            runtime,
            llm,
        }
    }

    pub async fn current_step(
        &mut self,
        request: &LlmStarshipMissionRequest,
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

        let work_items = response
            .neighbors
            .iter()
            .filter(|node| node.node_kind == "work_item")
            .map(|node| {
                json!({
                    "node_id": node.node_id,
                    "title": node.title,
                    "status": node.status,
                    "sequence": node.properties.get("sequence"),
                    "deliverables": node.properties.get("deliverables"),
                })
            })
            .collect::<Vec<_>>();

        let selected_step_node_id = match self.select_current_step_node_id(&work_items).await {
            Ok(node_id) => node_id,
            Err(error) => {
                debug_log("falling back to deterministic step selection after invalid LLM output");
                debug_log_value("llm selection error", error.to_string());
                deterministic_current_step_node_id(&response.neighbors)?
            }
        };

        response
            .neighbors
            .into_iter()
            .find(|node| node.node_id == selected_step_node_id)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("selected unknown step `{selected_step_node_id}`"),
                )
                .into()
            })
    }

    pub async fn execute_next_step(
        &mut self,
        request: LlmStarshipMissionRequest,
    ) -> RuntimeResult<LlmStarshipMissionExecution> {
        let step = self.current_step(&request).await?;
        debug_log_value("llm selected step", &step.node_id);

        let listed_files = self
            .runtime
            .invoke("fs.list", json!({ "path": "." }), false)
            .await?
            .output;

        let response = self
            .query_client
            .get_context(GetContextRequest {
                root_node_id: request.root_node_id,
                role: request.role,
                phase: 0,
                work_item_id: step.node_id.clone(),
                token_budget: 4000,
                requested_scopes: Vec::new(),
                render_format: 0,
                include_debug_sections: true,
            })
            .await?
            .into_inner();

        let bundle = response
            .bundle
            .ok_or_else(|| io::Error::other("missing bundle in LLM starship context"))?;
        let role_bundle = bundle
            .bundles
            .first()
            .ok_or_else(|| io::Error::other("missing role bundle in LLM starship context"))?;
        let detail = role_bundle
            .node_details
            .iter()
            .find(|detail| detail.node_id == step.node_id)
            .ok_or_else(|| io::Error::other("missing step detail in LLM starship context"))?;
        let rendered = response
            .rendered
            .ok_or_else(|| io::Error::other("missing rendered context in LLM starship context"))?;

        let deliverables = step
            .properties
            .get("deliverables")
            .map(|value| {
                value
                    .split(',')
                    .map(str::trim)
                    .filter(|entry| !entry.is_empty())
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let mut written_paths = Vec::new();
        let existing_paths = listed_files
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(ToString::to_string)
            .collect::<BTreeSet<_>>();
        for deliverable in deliverables
            .iter()
            .filter(|deliverable| !existing_paths.contains(*deliverable))
        {
            let generated: FileContent = self
                .llm
                .chat_json(
                    STARSHIP_FILE_SYSTEM_PROMPT,
                    &format!(
                        "Mission root: {}\nCurrent step id: {}\nCurrent step title: {}\nCurrent step detail: {}\nTarget deliverable path: {}\nAllowed deliverables for this step: {}\nExisting files:\n{}\nRendered context:\n{}\n\nReturn exactly one JSON object like {{\"content\":\"...\"}}. The content must be compact, plausible, and only for the requested path. Do not include explanations, markdown fences, or any other keys.",
                        role_bundle
                            .root_node
                            .as_ref()
                            .map(|node| node.title.clone())
                            .unwrap_or_else(|| "unknown".to_string()),
                        step.node_id,
                        step.title,
                        detail.detail,
                        deliverable,
                        serde_json::to_string(&deliverables).map_err(io::Error::other)?,
                        listed_files,
                        rendered.content,
                    ),
                )
                .await?;

            self.runtime
                .invoke(
                    "fs.write",
                    json!({
                        "path": deliverable,
                        "content": generated.content,
                    }),
                    true,
                )
                .await?;
            written_paths.push(deliverable.clone());
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

        Ok(LlmStarshipMissionExecution {
            selected_step_node_id: step.node_id,
            written_paths,
            captains_log,
        })
    }
}

impl<R> LlmStarshipMissionAgent<R>
where
    R: AgentRuntime,
{
    async fn select_current_step_node_id(
        &self,
        work_items: &[serde_json::Value],
    ) -> RuntimeResult<String> {
        let work_items_json = serde_json::to_string(work_items).map_err(io::Error::other)?;
        let allowed_ids = work_items
            .iter()
            .filter_map(|item| item.get("node_id").and_then(|value| value.as_str()))
            .collect::<Vec<_>>();
        let allowed_ids_json = serde_json::to_string(&allowed_ids).map_err(io::Error::other)?;

        let selection: StepSelection = self
            .llm
            .chat_json(
                STARSHIP_SELECTION_SYSTEM_PROMPT,
                &format!(
                    "Choose the single current step for the mission from these work items: {work_items_json}\nAllowed node ids: {allowed_ids_json}\nRules: prefer IN_PROGRESS; otherwise choose the first non-completed step by numeric sequence.\nReturn exactly one JSON object like {{\"selected_step_node_id\":\"one-of-the-allowed-node-ids\"}}."
                ),
            )
            .await?;

        if !allowed_ids
            .iter()
            .any(|candidate| *candidate == selection.selected_step_node_id)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "LLM selected unsupported step `{}`",
                    selection.selected_step_node_id
                ),
            )
            .into());
        }

        Ok(selection.selected_step_node_id)
    }
}

#[derive(Debug, Deserialize)]
struct StepSelection {
    selected_step_node_id: String,
}

#[derive(Debug, Deserialize)]
struct FileContent {
    content: String,
}

const STARSHIP_SELECTION_SYSTEM_PROMPT: &str = "You are selecting the current mission step for a rehydrating agent. Return strict JSON only: {\"selected_step_node_id\":\"...\"}.";

const STARSHIP_FILE_SYSTEM_PROMPT: &str = "You are continuing a coding mission after rehydration. Return strict JSON only: {\"content\":\"...\"}. Generate only the requested target file. Keep the file compact and plausible. Never return markdown fences or explanations. If the target is state/starship-state.json, the content itself must be valid JSON text. Never rewrite phase 1 deliverables during phase 2.";

fn deterministic_current_step_node_id(neighbors: &[GraphNode]) -> RuntimeResult<String> {
    let mut work_items = neighbors
        .iter()
        .filter(|node| node.node_kind == "work_item")
        .collect::<Vec<_>>();
    work_items.sort_by_key(|node| sequence_of(node));

    if let Some(node) = work_items
        .iter()
        .find(|node| node.status.eq_ignore_ascii_case("IN_PROGRESS"))
    {
        return Ok(node.node_id.clone());
    }

    if let Some(node) = work_items
        .iter()
        .find(|node| !node.status.eq_ignore_ascii_case("COMPLETED"))
    {
        return Ok(node.node_id.clone());
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "no selectable work_item nodes available",
    )
    .into())
}

fn sequence_of(node: &GraphNode) -> u32 {
    node.properties
        .get("sequence")
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(u32::MAX)
}
