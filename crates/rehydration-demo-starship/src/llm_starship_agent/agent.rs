use std::io;

use rehydration_proto::v1alpha1::{
    GraphNode, context_admin_service_client::ContextAdminServiceClient,
    context_query_service_client::ContextQueryServiceClient,
};
use serde::Deserialize;
use serde_json::json;
use tonic::transport::Channel;

use crate::logging::{debug_log, debug_log_value};
use crate::runtime_contract::{AgentRuntime, RuntimeResult};
use crate::{CAPTAINS_LOG_PATH, LlmPlanner};

use super::execution::LlmStarshipMissionExecution;
use super::file_generation::{
    build_file_generation_prompt, listed_paths, parse_deliverables, pending_deliverables,
    should_read_captains_log,
};
use super::request::{LlmStarshipMissionRequest, build_context_request};
use super::selection::{
    allowed_step_ids, build_selection_prompt, deterministic_current_step_node_id,
    ensure_supported_selection, work_item_candidates,
};

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
            .get_graph_relationships(rehydration_proto::v1alpha1::GetGraphRelationshipsRequest {
                node_id: request.root_node_id.clone(),
                node_kind: request.root_node_kind.clone(),
                depth: 1,
                include_reverse_edges: false,
            })
            .await?
            .into_inner();

        let work_items = work_item_candidates(&response.neighbors);

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
            .get_context(build_context_request(&request, &step.node_id))
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

        let deliverables = parse_deliverables(&step);
        let existing_paths = listed_paths(&listed_files);
        let root_title = role_bundle
            .root_node
            .as_ref()
            .map(|node| node.title.as_str())
            .unwrap_or("unknown");

        let mut written_paths = Vec::new();
        for deliverable in pending_deliverables(&deliverables, &existing_paths) {
            let generated: FileContent = self
                .llm
                .chat_json(
                    STARSHIP_FILE_SYSTEM_PROMPT,
                    &build_file_generation_prompt(
                        root_title,
                        &step,
                        &detail.detail,
                        deliverable,
                        &deliverables,
                        &listed_files,
                        &rendered.content,
                    )?,
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
            written_paths.push(deliverable.to_string());
        }

        let captains_log = if should_read_captains_log(&written_paths) {
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

    async fn select_current_step_node_id(
        &self,
        work_items: &[serde_json::Value],
    ) -> RuntimeResult<String> {
        let allowed_ids = allowed_step_ids(work_items);

        let selection: StepSelection = self
            .llm
            .chat_json(
                STARSHIP_SELECTION_SYSTEM_PROMPT,
                &build_selection_prompt(work_items)?,
            )
            .await?;

        ensure_supported_selection(&selection.selected_step_node_id, &allowed_ids)?;

        Ok(selection.selected_step_node_id)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, VecDeque};
    use std::sync::Arc;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use rehydration_proto::v1alpha1::{
        BundleNodeDetail, BundleRenderFormat, GetBundleSnapshotRequest, GetBundleSnapshotResponse,
        GetContextRequest, GetContextResponse, GetGraphRelationshipsRequest,
        GetGraphRelationshipsResponse, GetProjectionStatusRequest, GetProjectionStatusResponse,
        GetRehydrationDiagnosticsRequest, GetRehydrationDiagnosticsResponse, GraphNode,
        GraphRelationship, GraphRoleBundle, RehydrateSessionRequest, RehydrateSessionResponse,
        RehydrationBundle, RenderedContext, ReplayProjectionRequest, ReplayProjectionResponse,
        ValidateScopeRequest, ValidateScopeResponse,
        context_admin_service_client::ContextAdminServiceClient,
        context_admin_service_server::{ContextAdminService, ContextAdminServiceServer},
        context_query_service_client::ContextQueryServiceClient,
        context_query_service_server::{ContextQueryService, ContextQueryServiceServer},
    };
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::sync::Mutex;
    use tonic::{Request, Response, Status, transport::Server};

    use super::LlmStarshipMissionAgent;
    use crate::{FileSystemRuntime, LlmPlanner, OpenAiCompatClient, REPAIR_COMMAND_PATH};

    use super::super::request::LlmStarshipMissionRequest;

    #[tokio::test]
    async fn execute_next_step_uses_real_agent_module_with_fake_services() {
        let step = work_item_node(
            "node:work_item:repair",
            "Repair the hull",
            "IN_PROGRESS",
            REPAIR_COMMAND_PATH,
            "1",
        );
        let responses = vec![
            (
                200,
                format!(
                    r#"{{"choices":[{{"message":{{"content":"{{\"selected_step_node_id\":\"{}\"}}"}}}}]}}"#,
                    step.node_id
                ),
            ),
            (
                200,
                r#"{"choices":[{"message":{"content":"{\"content\":\"pub fn repair() -> &'static str {\\n    \\\"hull stabilized\\\"\\n}\\n\"}"}}]}"#
                    .to_string(),
            ),
        ];
        let llm = LlmPlanner::OpenAiCompat(
            OpenAiCompatClient::new(spawn_openai_server(responses).await, "demo-model", None)
                .expect("client should build"),
        );
        let workspace = temp_workspace();
        let runtime = FileSystemRuntime::new_for_test(&workspace);
        let request = LlmStarshipMissionRequest::reference_defaults("node:mission:root", "mission");
        let (query_client, admin_client) =
            spawn_services(query_response(step.clone()), graph_response(step.clone())).await;

        let mut agent = LlmStarshipMissionAgent::new(query_client, admin_client, runtime, llm);
        let execution = agent
            .execute_next_step(request)
            .await
            .expect("agent execution should succeed");

        assert_eq!(execution.selected_step_node_id, step.node_id);
        assert_eq!(
            execution.written_paths,
            vec![REPAIR_COMMAND_PATH.to_string()]
        );
        assert!(execution.captains_log.is_none());

        let written = std::fs::read_to_string(workspace.join(REPAIR_COMMAND_PATH))
            .expect("repair command should be written");
        assert!(written.contains("hull stabilized"));

        if workspace.exists() {
            std::fs::remove_dir_all(workspace).expect("workspace cleanup should succeed");
        }
    }

    #[tokio::test]
    async fn current_step_falls_back_when_llm_selects_unknown_step() {
        let in_progress = work_item_node(
            "node:work_item:repair",
            "Repair the hull",
            "IN_PROGRESS",
            REPAIR_COMMAND_PATH,
            "1",
        );
        let pending = work_item_node(
            "node:work_item:report",
            "Report the status",
            "PENDING",
            "captains-log.md",
            "2",
        );
        let llm = LlmPlanner::OpenAiCompat(
            OpenAiCompatClient::new(
                spawn_openai_server(vec![(
                    200,
                    r#"{"choices":[{"message":{"content":"{\"selected_step_node_id\":\"node:work_item:unknown\"}"}}]}"#
                        .to_string(),
                )])
                .await,
                "demo-model",
                None,
            )
            .expect("client should build"),
        );
        let workspace = temp_workspace();
        let runtime = FileSystemRuntime::new_for_test(&workspace);
        let request = LlmStarshipMissionRequest::reference_defaults("node:mission:root", "mission");
        let (query_client, admin_client) = spawn_services(
            query_response(in_progress.clone()),
            graph_response_with_neighbors(vec![in_progress.clone(), pending]),
        )
        .await;

        let mut agent = LlmStarshipMissionAgent::new(query_client, admin_client, runtime, llm);
        let selected = agent
            .current_step(&request)
            .await
            .expect("fallback selection should succeed");

        assert_eq!(selected.node_id, in_progress.node_id);

        if workspace.exists() {
            std::fs::remove_dir_all(workspace).expect("workspace cleanup should succeed");
        }
    }

    fn query_response(step: GraphNode) -> GetContextResponse {
        let root = GraphNode {
            node_id: "node:mission:root".to_string(),
            node_kind: "mission".to_string(),
            title: "Repair The Starship".to_string(),
            summary: "Restore the ship".to_string(),
            status: "ACTIVE".to_string(),
            labels: Vec::new(),
            properties: HashMap::new(),
        };

        GetContextResponse {
            bundle: Some(RehydrationBundle {
                root_node_id: root.node_id.clone(),
                bundles: vec![GraphRoleBundle {
                    role: "implementer".to_string(),
                    root_node: Some(root),
                    neighbor_nodes: vec![step.clone()],
                    relationships: vec![GraphRelationship {
                        source_node_id: "node:mission:root".to_string(),
                        target_node_id: step.node_id.clone(),
                        relationship_type: "contains".to_string(),
                        properties: HashMap::new(),
                    }],
                    node_details: vec![BundleNodeDetail {
                        node_id: step.node_id,
                        detail: "Repair the ship and persist the result".to_string(),
                        content_hash: "sha256:test".to_string(),
                        revision: 1,
                    }],
                }],
                stats: None,
                version: None,
            }),
            rendered: Some(RenderedContext {
                format: BundleRenderFormat::LlmPrompt as i32,
                content: "Rendered starship context".to_string(),
                token_count: 42,
                sections: Vec::new(),
            }),
            scope_validation: None,
            served_at: None,
        }
    }

    fn graph_response(step: GraphNode) -> GetGraphRelationshipsResponse {
        graph_response_with_neighbors(vec![step])
    }

    fn graph_response_with_neighbors(neighbors: Vec<GraphNode>) -> GetGraphRelationshipsResponse {
        GetGraphRelationshipsResponse {
            root: Some(GraphNode {
                node_id: "node:mission:root".to_string(),
                node_kind: "mission".to_string(),
                title: "Repair The Starship".to_string(),
                summary: String::new(),
                status: "ACTIVE".to_string(),
                labels: Vec::new(),
                properties: HashMap::new(),
            }),
            neighbors,
            relationships: Vec::new(),
            observed_at: None,
        }
    }

    fn work_item_node(
        node_id: &str,
        title: &str,
        status: &str,
        deliverable: &str,
        sequence: &str,
    ) -> GraphNode {
        GraphNode {
            node_id: node_id.to_string(),
            node_kind: "work_item".to_string(),
            title: title.to_string(),
            summary: String::new(),
            status: status.to_string(),
            labels: Vec::new(),
            properties: HashMap::from([
                ("deliverables".to_string(), deliverable.to_string()),
                ("sequence".to_string(), sequence.to_string()),
            ]),
        }
    }

    async fn spawn_services(
        query_response: GetContextResponse,
        graph_response: GetGraphRelationshipsResponse,
    ) -> (
        ContextQueryServiceClient<tonic::transport::Channel>,
        ContextAdminServiceClient<tonic::transport::Channel>,
    ) {
        let address = std::net::TcpListener::bind("127.0.0.1:0")
            .expect("listener should bind")
            .local_addr()
            .expect("listener should have address");

        let query_service = FakeQueryService {
            response: query_response,
        };
        let admin_service = FakeAdminService {
            response: graph_response,
        };
        tokio::spawn(async move {
            Server::builder()
                .add_service(ContextQueryServiceServer::new(query_service))
                .add_service(ContextAdminServiceServer::new(admin_service))
                .serve(address)
                .await
                .expect("test gRPC server should run");
        });

        tokio::time::sleep(Duration::from_millis(50)).await;

        let endpoint = format!("http://{address}");
        (
            ContextQueryServiceClient::connect(endpoint.clone())
                .await
                .expect("query client should connect"),
            ContextAdminServiceClient::connect(endpoint)
                .await
                .expect("admin client should connect"),
        )
    }

    async fn spawn_openai_server(responses: Vec<(u16, String)>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let address = listener.local_addr().expect("listener should have address");
        let responses = Arc::new(Mutex::new(VecDeque::from(responses)));

        tokio::spawn({
            let responses = Arc::clone(&responses);
            async move {
                loop {
                    let (mut socket, _) =
                        listener.accept().await.expect("connection should arrive");
                    let responses = Arc::clone(&responses);
                    tokio::spawn(async move {
                        let mut buffer = vec![0; 65_536];
                        let _ = socket.read(&mut buffer).await.expect("request should read");
                        let (status, body) = responses
                            .lock()
                            .await
                            .pop_front()
                            .unwrap_or((500, r#"{"error":"missing test response"}"#.to_string()));
                        let reason = if status == 200 { "OK" } else { "ERROR" };
                        let response = format!(
                            "HTTP/1.1 {status} {reason}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                            body.len()
                        );
                        socket
                            .write_all(response.as_bytes())
                            .await
                            .expect("response should write");
                    });
                }
            }
        });

        format!("http://{address}")
    }

    fn temp_workspace() -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "rehydration-starship-demo-agent-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock should work")
                .as_millis()
        ))
    }

    #[derive(Clone)]
    struct FakeQueryService {
        response: GetContextResponse,
    }

    #[tonic::async_trait]
    impl ContextQueryService for FakeQueryService {
        async fn get_context(
            &self,
            request: Request<GetContextRequest>,
        ) -> Result<Response<GetContextResponse>, Status> {
            let request = request.into_inner();
            if request.root_node_id != "node:mission:root" {
                return Err(Status::invalid_argument("unexpected root node id"));
            }
            Ok(Response::new(self.response.clone()))
        }

        async fn rehydrate_session(
            &self,
            _request: Request<RehydrateSessionRequest>,
        ) -> Result<Response<RehydrateSessionResponse>, Status> {
            Err(Status::unimplemented("not needed in test"))
        }

        async fn validate_scope(
            &self,
            _request: Request<ValidateScopeRequest>,
        ) -> Result<Response<ValidateScopeResponse>, Status> {
            Err(Status::unimplemented("not needed in test"))
        }
    }

    #[derive(Clone)]
    struct FakeAdminService {
        response: GetGraphRelationshipsResponse,
    }

    #[tonic::async_trait]
    impl ContextAdminService for FakeAdminService {
        async fn get_projection_status(
            &self,
            _request: Request<GetProjectionStatusRequest>,
        ) -> Result<Response<GetProjectionStatusResponse>, Status> {
            Err(Status::unimplemented("not needed in test"))
        }

        async fn replay_projection(
            &self,
            _request: Request<ReplayProjectionRequest>,
        ) -> Result<Response<ReplayProjectionResponse>, Status> {
            Err(Status::unimplemented("not needed in test"))
        }

        async fn get_bundle_snapshot(
            &self,
            _request: Request<GetBundleSnapshotRequest>,
        ) -> Result<Response<GetBundleSnapshotResponse>, Status> {
            Err(Status::unimplemented("not needed in test"))
        }

        async fn get_graph_relationships(
            &self,
            request: Request<GetGraphRelationshipsRequest>,
        ) -> Result<Response<GetGraphRelationshipsResponse>, Status> {
            let request = request.into_inner();
            if request.node_id != "node:mission:root" {
                return Err(Status::invalid_argument("unexpected graph root"));
            }
            Ok(Response::new(self.response.clone()))
        }

        async fn get_rehydration_diagnostics(
            &self,
            _request: Request<GetRehydrationDiagnosticsRequest>,
        ) -> Result<Response<GetRehydrationDiagnosticsResponse>, Status> {
            Err(Status::unimplemented("not needed in test"))
        }
    }
}
