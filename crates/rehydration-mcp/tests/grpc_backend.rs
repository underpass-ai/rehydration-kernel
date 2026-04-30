use std::collections::HashMap;

use rehydration_mcp::KernelMcpServer;
use rehydration_proto::v1beta1::{
    BundleNodeDetail, BundleRenderFormat, BundleSection, GetContextPathRequest,
    GetContextPathResponse, GetContextRequest, GetContextResponse, GetNodeDetailRequest,
    GetNodeDetailResponse, GraphNode, GraphRelationship, GraphRelationshipExplanation,
    GraphRelationshipSemanticClass, GraphRoleBundle, RehydrateSessionRequest,
    RehydrateSessionResponse, RehydrationBundle, RehydrationMode, RenderedContext,
    ValidateScopeRequest, ValidateScopeResponse,
    context_query_service_server::{ContextQueryService, ContextQueryServiceServer},
};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::{Request, Response, Status};

#[tokio::test]
async fn grpc_backend_maps_live_query_service_responses_to_kmp_tools() {
    let endpoint = spawn_fake_query_server().await;
    let server = KernelMcpServer::grpc(endpoint);

    let wake = call_tool(
        &server,
        1,
        "kernel_wake",
        json!({
            "about": "node:root",
            "role": "implementer",
            "intent": "continue the live incident",
            "depth": 3,
            "budget": {
                "tokens": 321
            }
        }),
    )
    .await;
    assert_eq!(wake["result"]["isError"], false);
    assert_eq!(
        wake["result"]["structuredContent"]["wake"]["objective"],
        "continue the live incident"
    );
    assert_eq!(
        wake["result"]["structuredContent"]["wake"]["current_state"][0],
        "State: Live state for node:root as implementer at depth 3 with budget 321."
    );
    assert_eq!(
        wake["result"]["structuredContent"]["proof"]["path"][0]["class"],
        "evidential"
    );
    assert_eq!(
        wake["result"]["structuredContent"]["proof"]["evidence"][0]["id"],
        "detail:node:root:evidence"
    );

    let ask = call_tool(
        &server,
        2,
        "kernel_ask",
        json!({
            "about": "node:root",
            "question": "What should the next agent trust?",
            "budget": {
                "tokens": 654
            }
        }),
    )
    .await;
    assert_eq!(ask["result"]["isError"], false);
    assert_eq!(ask["result"]["structuredContent"]["answer"], Value::Null);
    assert_eq!(
        ask["result"]["structuredContent"]["because"][0]["evidence"],
        "Evidence detail for node:root requested by answerer with budget 654."
    );
    assert_eq!(
        ask["result"]["structuredContent"]["proof"]["missing"][0],
        "generative_answer"
    );

    let trace = call_tool(
        &server,
        3,
        "kernel_trace",
        json!({
            "from": "node:root",
            "to": "node:target",
            "role": "auditor",
            "budget": {
                "tokens": 111
            }
        }),
    )
    .await;
    assert_eq!(trace["result"]["isError"], false);
    assert_eq!(
        trace["result"]["structuredContent"]["summary"],
        "Live state for node:root as auditor at depth 1 with budget 111."
    );
    assert_eq!(
        trace["result"]["structuredContent"]["trace"][0]["from"],
        "node:root"
    );
    assert_eq!(
        trace["result"]["structuredContent"]["trace"][0]["to"],
        "node:target"
    );
    assert_eq!(
        trace["result"]["structuredContent"]["trace"][0]["rel"],
        "supports"
    );

    let inspect = call_tool(
        &server,
        4,
        "kernel_inspect",
        json!({
            "ref": "node:target"
        }),
    )
    .await;
    assert_eq!(inspect["result"]["isError"], false);
    assert_eq!(
        inspect["result"]["structuredContent"]["object"]["ref"],
        "node:target"
    );
    assert_eq!(
        inspect["result"]["structuredContent"]["object"]["kind"],
        "claim"
    );
    assert_eq!(
        inspect["result"]["structuredContent"]["evidence"][0]["text"],
        "Node detail for node:target."
    );
}

async fn spawn_fake_query_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("fake gRPC server should bind to an ephemeral port");
    let addr = listener
        .local_addr()
        .expect("fake gRPC server should expose its local address");
    let incoming = TcpListenerStream::new(listener);

    tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(ContextQueryServiceServer::new(FakeQueryService))
            .serve_with_incoming(incoming)
            .await
            .expect("fake gRPC server should run");
    });

    format!("http://{addr}")
}

async fn call_tool(server: &KernelMcpServer, id: u64, name: &str, arguments: Value) -> Value {
    let response = server
        .handle_json_line(
            &json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "tools/call",
                "params": {
                    "name": name,
                    "arguments": arguments
                }
            })
            .to_string(),
        )
        .await
        .expect("tools/call should produce a response");

    serde_json::from_str(&response).expect("tools/call response should be valid JSON")
}

struct FakeQueryService;

#[tonic::async_trait]
impl ContextQueryService for FakeQueryService {
    async fn get_context(
        &self,
        request: Request<GetContextRequest>,
    ) -> Result<Response<GetContextResponse>, Status> {
        let request = request.into_inner();
        Ok(Response::new(GetContextResponse {
            bundle: Some(fake_bundle(
                &request.root_node_id,
                "node:root:evidence",
                &request.role,
                request.depth,
                request.token_budget,
            )),
            rendered: Some(fake_rendered(
                &request.root_node_id,
                &request.role,
                request.depth,
                request.token_budget,
            )),
            scope_validation: None,
            served_at: None,
            timing: None,
        }))
    }

    async fn get_context_path(
        &self,
        request: Request<GetContextPathRequest>,
    ) -> Result<Response<GetContextPathResponse>, Status> {
        let request = request.into_inner();
        Ok(Response::new(GetContextPathResponse {
            path_bundle: Some(fake_path_bundle(
                &request.root_node_id,
                &request.target_node_id,
                &request.role,
                request.token_budget,
            )),
            rendered: Some(fake_rendered(
                &request.root_node_id,
                &request.role,
                1,
                request.token_budget,
            )),
            served_at: None,
            timing: None,
        }))
    }

    async fn get_node_detail(
        &self,
        request: Request<GetNodeDetailRequest>,
    ) -> Result<Response<GetNodeDetailResponse>, Status> {
        let node_id = request.into_inner().node_id;
        Ok(Response::new(GetNodeDetailResponse {
            node: Some(fake_node(&node_id)),
            detail: Some(BundleNodeDetail {
                node_id: node_id.clone(),
                detail: format!("Node detail for {node_id}."),
                content_hash: "sha256:inspect".to_string(),
                revision: 1,
            }),
        }))
    }

    async fn rehydrate_session(
        &self,
        _request: Request<RehydrateSessionRequest>,
    ) -> Result<Response<RehydrateSessionResponse>, Status> {
        Err(Status::unimplemented(
            "fake query service only implements KMP read paths",
        ))
    }

    async fn validate_scope(
        &self,
        _request: Request<ValidateScopeRequest>,
    ) -> Result<Response<ValidateScopeResponse>, Status> {
        Err(Status::unimplemented(
            "fake query service only implements KMP read paths",
        ))
    }
}

fn fake_bundle(
    root_node_id: &str,
    evidence_node_id: &str,
    role: &str,
    depth: u32,
    token_budget: u32,
) -> RehydrationBundle {
    RehydrationBundle {
        root_node_id: root_node_id.to_string(),
        bundles: vec![GraphRoleBundle {
            role: role.to_string(),
            root_node: Some(fake_node(root_node_id)),
            neighbor_nodes: vec![fake_node(evidence_node_id)],
            relationships: vec![fake_relationship(root_node_id, evidence_node_id)],
            node_details: vec![BundleNodeDetail {
                node_id: evidence_node_id.to_string(),
                detail: format!(
                    "Evidence detail for {root_node_id} requested by {role} with budget {token_budget}."
                ),
                content_hash: format!("sha256:{root_node_id}:{depth}:{token_budget}"),
                revision: 7,
            }],
            rendered: None,
        }],
        stats: None,
        version: None,
    }
}

fn fake_path_bundle(
    root_node_id: &str,
    target_node_id: &str,
    role: &str,
    token_budget: u32,
) -> RehydrationBundle {
    RehydrationBundle {
        root_node_id: root_node_id.to_string(),
        bundles: vec![GraphRoleBundle {
            role: role.to_string(),
            root_node: Some(fake_node(root_node_id)),
            neighbor_nodes: vec![fake_node(target_node_id)],
            relationships: vec![fake_relationship(root_node_id, target_node_id)],
            node_details: vec![BundleNodeDetail {
                node_id: target_node_id.to_string(),
                detail: format!("Path detail for {root_node_id} to {target_node_id}."),
                content_hash: format!("sha256:path:{token_budget}"),
                revision: 2,
            }],
            rendered: None,
        }],
        stats: None,
        version: None,
    }
}

fn fake_rendered(root_node_id: &str, role: &str, depth: u32, token_budget: u32) -> RenderedContext {
    let content = format!(
        "Live state for {root_node_id} as {role} at depth {depth} with budget {token_budget}."
    );

    RenderedContext {
        format: BundleRenderFormat::Structured as i32,
        content: content.clone(),
        token_count: 12,
        sections: vec![BundleSection {
            key: "state".to_string(),
            title: "State".to_string(),
            content,
            token_count: 12,
            scopes: Vec::new(),
        }],
        tiers: Vec::new(),
        resolved_mode: RehydrationMode::ResumeFocused as i32,
        quality: None,
        truncation: None,
        content_hash: "sha256:rendered".to_string(),
    }
}

fn fake_node(node_id: &str) -> GraphNode {
    GraphNode {
        node_id: node_id.to_string(),
        node_kind: "claim".to_string(),
        title: format!("Claim {node_id}"),
        summary: format!("Summary for {node_id}."),
        status: "active".to_string(),
        labels: vec!["test".to_string()],
        properties: HashMap::new(),
        provenance: None,
    }
}

fn fake_relationship(source_node_id: &str, target_node_id: &str) -> GraphRelationship {
    GraphRelationship {
        source_node_id: source_node_id.to_string(),
        target_node_id: target_node_id.to_string(),
        relationship_type: "supports".to_string(),
        explanation: Some(GraphRelationshipExplanation {
            semantic_class: GraphRelationshipSemanticClass::Evidential as i32,
            rationale: format!("{source_node_id} is supported by {target_node_id}."),
            motivation: String::new(),
            method: String::new(),
            decision_id: "decision:test".to_string(),
            caused_by_node_id: String::new(),
            evidence: format!("Evidence connects {source_node_id} to {target_node_id}."),
            confidence: "high".to_string(),
            sequence: 1,
        }),
        provenance: None,
    }
}
