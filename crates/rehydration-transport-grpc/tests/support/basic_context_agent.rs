use std::error::Error;
use std::io;

use rehydration_proto::v1alpha1::{
    BundleRenderFormat, GetContextRequest, GetGraphRelationshipsRequest, GraphNode, Phase,
    context_admin_service_client::ContextAdminServiceClient,
    context_query_service_client::ContextQueryServiceClient,
};
use serde_json::json;
use tonic::transport::Channel;

use crate::agentic_support::agentic_debug::{debug_log, debug_log_value};
use crate::agentic_support::runtime_workspace::{AgentRuntime, RuntimeResult, ToolDescriptor};

pub(crate) const SUMMARY_PATH: &str = "context-summary.md";

#[derive(Debug)]
pub(crate) struct AgentExecution {
    pub(crate) selected_node_id: String,
    pub(crate) written_path: String,
    pub(crate) written_content: String,
    pub(crate) listed_files: String,
}

pub(crate) struct BasicContextAgent<R> {
    query_client: ContextQueryServiceClient<Channel>,
    admin_client: ContextAdminServiceClient<Channel>,
    runtime: R,
}

impl<R> BasicContextAgent<R>
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

    pub(crate) fn runtime(&self) -> &R {
        &self.runtime
    }

    pub(crate) async fn execute(
        &mut self,
        root_node_id: &str,
        root_node_kind: &str,
    ) -> Result<AgentExecution, Box<dyn Error + Send + Sync>> {
        debug_log_value("agent root_node_id", root_node_id);
        debug_log_value("agent root_node_kind", root_node_kind);
        let tools = self.runtime.list_tools().await?;
        debug_log_value(
            "agent tools",
            tools
                .iter()
                .map(|tool| tool.name.as_str())
                .collect::<Vec<_>>()
                .join(","),
        );
        ensure_runtime_supports_context_workflow(&tools)?;

        let focus_node = self.select_focus_node(root_node_id, root_node_kind).await?;
        debug_log_value("selected focus node", &focus_node.node_id);
        let response = self
            .query_client
            .get_context(GetContextRequest {
                root_node_id: root_node_id.to_string(),
                role: "implementer".to_string(),
                phase: Phase::Build as i32,
                work_item_id: focus_node.node_id.clone(),
                token_budget: 1200,
                requested_scopes: vec!["implementation".to_string(), "dependencies".to_string()],
                render_format: BundleRenderFormat::LlmPrompt as i32,
                include_debug_sections: false,
            })
            .await?
            .into_inner();
        debug_log("get_context response received");

        let bundle = response
            .bundle
            .ok_or_else(|| io::Error::other("missing bundle in get_context response"))?;
        let role_bundle = bundle
            .bundles
            .first()
            .ok_or_else(|| io::Error::other("missing role bundle in get_context response"))?;
        let focus_detail = role_bundle
            .node_details
            .iter()
            .find(|detail| detail.node_id == focus_node.node_id)
            .ok_or_else(|| io::Error::other("missing focused node detail"))?;
        let rendered = response
            .rendered
            .ok_or_else(|| io::Error::other("missing rendered context in get_context response"))?;

        let summary = format!(
            "# Context Summary\n\nRoot: {}\nFocus: {}\n\nDetail:\n{}\n\nRendered:\n{}\n",
            role_bundle
                .root_node
                .as_ref()
                .map(|node| node.title.as_str())
                .unwrap_or("unknown"),
            focus_node.title,
            focus_detail.detail,
            rendered.content,
        );
        debug_log_value("summary bytes", summary.len());

        self.runtime
            .invoke(
                "fs.write",
                json!({
                    "path": SUMMARY_PATH,
                    "content": summary,
                }),
                true,
            )
            .await?;
        debug_log("runtime fs.write completed");

        let read_back = self
            .runtime
            .invoke("fs.read", json!({ "path": SUMMARY_PATH }), false)
            .await?;
        debug_log_value("runtime fs.read bytes", read_back.output.len());
        let listed_files = self
            .runtime
            .invoke("fs.list", json!({ "path": "." }), false)
            .await?;
        debug_log_value("runtime fs.list output", &listed_files.output);

        Ok(AgentExecution {
            selected_node_id: focus_node.node_id,
            written_path: SUMMARY_PATH.to_string(),
            written_content: read_back.output,
            listed_files: listed_files.output,
        })
    }

    async fn select_focus_node(
        &mut self,
        root_node_id: &str,
        root_node_kind: &str,
    ) -> Result<GraphNode, Box<dyn Error + Send + Sync>> {
        let response = self
            .admin_client
            .get_graph_relationships(GetGraphRelationshipsRequest {
                node_id: root_node_id.to_string(),
                node_kind: root_node_kind.to_string(),
                depth: 1,
                include_reverse_edges: false,
            })
            .await?
            .into_inner();
        debug_log_value("graph neighbors count", response.neighbors.len());

        response
            .neighbors
            .into_iter()
            .find(|node| node.node_kind == "work_item")
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotFound, "missing work_item neighbor").into()
            })
    }
}

fn ensure_runtime_supports_context_workflow(tools: &[ToolDescriptor]) -> RuntimeResult<()> {
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
