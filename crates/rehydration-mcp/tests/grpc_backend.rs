use std::collections::HashMap;

use std::path::PathBuf;
use std::sync::Arc;

use rehydration_mcp::{KernelMcpGrpcTlsConfig, KernelMcpServer};
use rehydration_proto::v1beta1::{
    BundleNodeDetail, BundleRenderFormat, BundleSection, BundleVersion, GetContextPathRequest,
    GetContextPathResponse, GetContextRequest, GetContextResponse, GetNodeDetailRequest,
    GetNodeDetailResponse, GraphNode, GraphRelationship, GraphRelationshipExplanation,
    GraphRelationshipSemanticClass, GraphRoleBundle, RehydrateSessionRequest,
    RehydrateSessionResponse, RehydrationBundle, RehydrationMode, RenderedContext,
    UpdateContextRequest, UpdateContextResponse, ValidateScopeRequest, ValidateScopeResponse,
    context_command_service_server::{ContextCommandService, ContextCommandServiceServer},
    context_query_service_server::{ContextQueryService, ContextQueryServiceServer},
};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::{Certificate, Identity, ServerTlsConfig};
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

#[tokio::test]
async fn grpc_backend_maps_kernel_ingest_to_update_context() {
    let recorded_commands = RecordedCommands::default();
    let endpoint = spawn_fake_kernel_server_with_commands(recorded_commands.clone()).await;
    let server = KernelMcpServer::grpc(endpoint);

    let ingest = call_tool(
        &server,
        5,
        "kernel_ingest",
        json!({
            "about": "question:830ce83f",
            "memory": {
                "dimensions": [
                    {
                        "id": "conversation:rachel",
                        "kind": "conversation"
                    }
                ],
                "entries": [
                    {
                        "id": "claim:rachel-austin",
                        "kind": "claim",
                        "text": "Rachel moved to Austin.",
                        "coordinates": [
                            {
                                "dimension": "conversation",
                                "scope_id": "conversation:rachel",
                                "sequence": 1
                            }
                        ]
                    }
                ],
                "relations": [
                    {
                        "from": "claim:rachel-austin",
                        "to": "claim:rachel-denver",
                        "rel": "supersedes",
                        "class": "evidential",
                        "why": "The later statement corrects the earlier one.",
                        "evidence": "Rachel corrected the destination.",
                        "confidence": "high"
                    }
                ],
                "evidence": [
                    {
                        "id": "evidence:rachel-turn-2",
                        "supports": ["claim:rachel-austin"],
                        "text": "Rachel corrected the destination.",
                        "source": "conversation turn 2"
                    }
                ]
            },
            "provenance": {
                "source_agent": "longmemeval-adapter",
                "correlation_id": "corr:830ce83f",
                "causation_id": "eval:item:830ce83f"
            },
            "idempotency_key": "ingest:830ce83f:1"
        }),
    )
    .await;

    assert_eq!(ingest["result"]["isError"], false);
    assert_eq!(
        ingest["result"]["structuredContent"]["memory"]["memory_id"],
        "memory:830ce83f:1"
    );
    assert_eq!(
        ingest["result"]["structuredContent"]["memory"]["accepted"]["entries"],
        1
    );
    assert_eq!(
        ingest["result"]["structuredContent"]["memory"]["read_after_write_ready"],
        true
    );

    let commands = recorded_commands.requests().await;
    assert_eq!(commands.len(), 1);
    let command = &commands[0];
    assert_eq!(command.root_node_id, "question:830ce83f");
    assert_eq!(command.role, "memory");
    assert_eq!(command.work_item_id, "ingest:830ce83f:1");
    assert_eq!(command.changes.len(), 4);
    assert_eq!(command.changes[0].entity_kind, "memory_dimension");
    assert_eq!(command.changes[1].entity_kind, "memory_entry");
    assert_eq!(command.changes[2].entity_kind, "memory_relation");
    assert_eq!(command.changes[3].entity_kind, "memory_evidence");
    assert_eq!(
        command
            .metadata
            .as_ref()
            .expect("ingest should set command metadata")
            .idempotency_key,
        "ingest:830ce83f:1"
    );
}

#[tokio::test]
async fn grpc_backend_dry_run_ingest_does_not_call_update_context() {
    let recorded_commands = RecordedCommands::default();
    let endpoint = spawn_fake_kernel_server_with_commands(recorded_commands.clone()).await;
    let server = KernelMcpServer::grpc(endpoint);

    let ingest = call_tool(
        &server,
        6,
        "kernel_ingest",
        json!({
            "about": "question:dry-run",
            "memory": {
                "dimensions": [{"id": "conversation:dry-run"}],
                "entries": [{"id": "claim:dry-run", "text": "Dry run memory."}]
            },
            "idempotency_key": "ingest:dry-run:1",
            "dry_run": true
        }),
    )
    .await;

    assert_eq!(ingest["result"]["isError"], false);
    assert!(
        ingest["result"]["structuredContent"]["warnings"][0]
            .as_str()
            .expect("dry run warning should be text")
            .contains("dry_run=true")
    );
    assert!(recorded_commands.requests().await.is_empty());
}

#[tokio::test]
async fn grpc_backend_connects_to_mutual_tls_query_service() {
    install_test_crypto_provider();

    let certs = TestTlsFiles::write();
    let endpoint = spawn_fake_query_server_with_mutual_tls().await;
    let server = KernelMcpServer::grpc_with_tls(endpoint, certs.client_config());

    let inspect = call_tool(
        &server,
        1,
        "kernel_inspect",
        json!({
            "ref": "node:mtls"
        }),
    )
    .await;

    assert_eq!(inspect["result"]["isError"], false);
    assert_eq!(
        inspect["result"]["structuredContent"]["object"]["ref"],
        "node:mtls"
    );
    assert_eq!(
        inspect["result"]["structuredContent"]["evidence"][0]["text"],
        "Node detail for node:mtls."
    );
}

async fn spawn_fake_query_server() -> String {
    spawn_fake_query_server_with_tls(None).await
}

async fn spawn_fake_query_server_with_mutual_tls() -> String {
    spawn_fake_query_server_with_tls(Some(
        ServerTlsConfig::new()
            .identity(Identity::from_pem(TEST_SERVER_CERT, TEST_SERVER_KEY))
            .client_ca_root(Certificate::from_pem(TEST_CA_CERT)),
    ))
    .await
}

async fn spawn_fake_query_server_with_tls(tls_config: Option<ServerTlsConfig>) -> String {
    spawn_fake_kernel_server_with_tls(tls_config, RecordedCommands::default()).await
}

async fn spawn_fake_kernel_server_with_commands(recorded_commands: RecordedCommands) -> String {
    spawn_fake_kernel_server_with_tls(None, recorded_commands).await
}

async fn spawn_fake_kernel_server_with_tls(
    tls_config: Option<ServerTlsConfig>,
    recorded_commands: RecordedCommands,
) -> String {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("fake gRPC server should bind to an ephemeral port");
    let addr = listener
        .local_addr()
        .expect("fake gRPC server should expose its local address");
    let incoming = TcpListenerStream::new(listener);

    tokio::spawn(async move {
        let mut builder = tonic::transport::Server::builder();
        if let Some(tls_config) = tls_config {
            builder = builder
                .tls_config(tls_config)
                .expect("fake gRPC server TLS config should be valid");
        }

        builder
            .add_service(ContextQueryServiceServer::new(FakeQueryService))
            .add_service(ContextCommandServiceServer::new(FakeCommandService {
                recorded_commands,
            }))
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

#[derive(Clone, Default)]
struct RecordedCommands {
    requests: Arc<Mutex<Vec<UpdateContextRequest>>>,
}

impl RecordedCommands {
    async fn requests(&self) -> Vec<UpdateContextRequest> {
        self.requests.lock().await.clone()
    }
}

#[derive(Clone)]
struct FakeCommandService {
    recorded_commands: RecordedCommands,
}

#[tonic::async_trait]
impl ContextCommandService for FakeCommandService {
    async fn update_context(
        &self,
        request: Request<UpdateContextRequest>,
    ) -> Result<Response<UpdateContextResponse>, Status> {
        let request = request.into_inner();
        self.recorded_commands
            .requests
            .lock()
            .await
            .push(request.clone());

        Ok(Response::new(UpdateContextResponse {
            accepted_version: Some(BundleVersion {
                revision: 1,
                content_hash: "sha256:ingest".to_string(),
                schema_version: "v1beta1".to_string(),
                projection_watermark: String::new(),
                generated_at: None,
                generator_version: "test".to_string(),
            }),
            warnings: Vec::new(),
        }))
    }
}

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

struct TestTlsFiles {
    _dir: tempfile::TempDir,
    ca_path: PathBuf,
    cert_path: PathBuf,
    key_path: PathBuf,
}

impl TestTlsFiles {
    fn write() -> Self {
        let dir = tempfile::tempdir().expect("test TLS tempdir should be created");
        let ca_path = dir.path().join("ca.crt");
        let cert_path = dir.path().join("client.crt");
        let key_path = dir.path().join("client.key");

        std::fs::write(&ca_path, TEST_CA_CERT).expect("test CA should be written");
        std::fs::write(&cert_path, TEST_CLIENT_CERT).expect("test client cert should be written");
        std::fs::write(&key_path, TEST_CLIENT_KEY).expect("test client key should be written");

        Self {
            _dir: dir,
            ca_path,
            cert_path,
            key_path,
        }
    }

    fn client_config(&self) -> KernelMcpGrpcTlsConfig {
        KernelMcpGrpcTlsConfig::mutual(
            self.ca_path.clone(),
            self.cert_path.clone(),
            self.key_path.clone(),
            Some("localhost".to_string()),
        )
    }
}

fn install_test_crypto_provider() {
    let _ = tokio_rustls::rustls::crypto::aws_lc_rs::default_provider().install_default();
}

// Static test-only certificates generated for the local mTLS fake server.
// They are not trusted by production code and exist only to avoid shelling out
// to OpenSSL during `cargo test`.
const TEST_CA_CERT: &str = r#"-----BEGIN CERTIFICATE-----
MIIBmDCCAT+gAwIBAgIUDrShLER4kZWk+jp6yLZMPVnc4KAwCgYIKoZIzj0EAwIw
IjEgMB4GA1UEAwwXcmVoeWRyYXRpb24tbWNwLXRlc3QtY2EwHhcNMjYwNTAzMTc1
NTMyWhcNMzYwNDMwMTc1NTMyWjAiMSAwHgYDVQQDDBdyZWh5ZHJhdGlvbi1tY3At
dGVzdC1jYTBZMBMGByqGSM49AgEGCCqGSM49AwEHA0IABARF7qIfl/QvSiZvX8di
hbYIuq/gzFG6qNa0v86901BvKM21S9zna0xfZbxodZd7mfwzvSDnHFJIAfymQ/a0
SSmjUzBRMB0GA1UdDgQWBBSDc7yfQDotnauGfYkkeHrMc+gSBjAfBgNVHSMEGDAW
gBSDc7yfQDotnauGfYkkeHrMc+gSBjAPBgNVHRMBAf8EBTADAQH/MAoGCCqGSM49
BAMCA0cAMEQCIH0m3WBS/qGq20lcXg+iO+RFHkE768JcjuB/viffYkzuAiAZczMl
OKRQEYO/3ZfaPZkHZsL99dUFy3czK6cAtly4Lg==
-----END CERTIFICATE-----
"#;

const TEST_SERVER_CERT: &str = r#"-----BEGIN CERTIFICATE-----
MIIBpTCCAUugAwIBAgIUDt2LOgaFR696iyRVM+ipq3EAEbcwCgYIKoZIzj0EAwIw
IjEgMB4GA1UEAwwXcmVoeWRyYXRpb24tbWNwLXRlc3QtY2EwHhcNMjYwNTAzMTc1
NjI3WhcNMzYwNDMwMTc1NjI3WjAUMRIwEAYDVQQDDAlsb2NhbGhvc3QwWTATBgcq
hkjOPQIBBggqhkjOPQMBBwNCAAQZadG+mSPC5wBjpC4V3TUX7ZGXJ8ypWKo6Pmah
zDGI2jVtXosZKtYkT7mrykgyO/U5lBMi4j6FZBR3ScXEYdzho20wazAUBgNVHREE
DTALgglsb2NhbGhvc3QwEwYDVR0lBAwwCgYIKwYBBQUHAwEwHQYDVR0OBBYEFAvZ
kmZyjsv0PhtiTOLZa2qQJhdbMB8GA1UdIwQYMBaAFINzvJ9AOi2dq4Z9iSR4esxz
6BIGMAoGCCqGSM49BAMCA0gAMEUCIEJHCWnCaKr73gefBhAYNZQjPUh5IomVAI0F
czqsWVUnAiEAyv098AZ9D0VKjWCNdfu+Q0jg96A4BD3SG+122qXiQgg=
-----END CERTIFICATE-----
"#;

const TEST_SERVER_KEY: &str = r#"-----BEGIN PRIVATE KEY-----
MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQg0ShbEuRisGPwMym1
C0vGZ/hD1c91p9fA5nvtWKoNRYahRANCAAQZadG+mSPC5wBjpC4V3TUX7ZGXJ8yp
WKo6PmahzDGI2jVtXosZKtYkT7mrykgyO/U5lBMi4j6FZBR3ScXEYdzh
-----END PRIVATE KEY-----
"#;

const TEST_CLIENT_CERT: &str = r#"-----BEGIN CERTIFICATE-----
MIIBoTCCAUegAwIBAgIUDt2LOgaFR696iyRVM+ipq3EAEbYwCgYIKoZIzj0EAwIw
IjEgMB4GA1UEAwwXcmVoeWRyYXRpb24tbWNwLXRlc3QtY2EwHhcNMjYwNTAzMTc1
NjAzWhcNMzYwNDMwMTc1NjAzWjAmMSQwIgYDVQQDDBtyZWh5ZHJhdGlvbi1tY3At
dGVzdC1jbGllbnQwWTATBgcqhkjOPQIBBggqhkjOPQMBBwNCAARbE50GqAcOulov
ZQLg9fRjq1LTMvjC3mi/wb+E6QDWGw/nobEuShCwGt8UhaXv6x1iOzlwDt1pBPYi
F9bXka0ho1cwVTATBgNVHSUEDDAKBggrBgEFBQcDAjAdBgNVHQ4EFgQU93T3BJ6O
zaMFcyOIBZ9xKPxNnC0wHwYDVR0jBBgwFoAUg3O8n0A6LZ2rhn2JJHh6zHPoEgYw
CgYIKoZIzj0EAwIDSAAwRQIgfG8Rt+loR9K/khgov0WMDLcMngb4y1aimUp92r+0
l50CIQDuNxk8bkC5XbTX7JP29dAkyrc55Pf728d08jt1TlRQjw==
-----END CERTIFICATE-----
"#;

const TEST_CLIENT_KEY: &str = r#"-----BEGIN PRIVATE KEY-----
MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgNuVBGBwRhxnlxsLV
qdYqw8bDQbReT4w+psRqj/cT1buhRANCAARbE50GqAcOulovZQLg9fRjq1LTMvjC
3mi/wb+E6QDWGw/nobEuShCwGt8UhaXv6x1iOzlwDt1pBPYiF9bXka0h
-----END PRIVATE KEY-----
"#;
