use std::fs;
use std::path::PathBuf;
use std::sync::Once;

use rehydration_proto::v1beta1::{
    BundleNodeDetail, GetContextPathRequest, GetContextRequest, GetContextResponse,
    GetNodeDetailRequest, GetNodeDetailResponse, GraphRelationship, GraphRelationshipSemanticClass,
    RehydrationMode, RenderedContext, ResolutionTier,
    context_query_service_client::ContextQueryServiceClient,
};
use serde_json::{Value, json};
use tonic::transport::{Certificate, ClientTlsConfig, Endpoint, Identity};

pub const GRPC_ENDPOINT_ENV: &str = "REHYDRATION_KERNEL_GRPC_ENDPOINT";
pub const GRPC_TLS_MODE_ENV: &str = "REHYDRATION_KERNEL_GRPC_TLS_MODE";
pub const GRPC_TLS_CA_PATH_ENV: &str = "REHYDRATION_KERNEL_GRPC_TLS_CA_PATH";
pub const GRPC_TLS_CERT_PATH_ENV: &str = "REHYDRATION_KERNEL_GRPC_TLS_CERT_PATH";
pub const GRPC_TLS_KEY_PATH_ENV: &str = "REHYDRATION_KERNEL_GRPC_TLS_KEY_PATH";
pub const GRPC_TLS_DOMAIN_NAME_ENV: &str = "REHYDRATION_KERNEL_GRPC_TLS_DOMAIN_NAME";

const PROTOCOL_VERSION: &str = "2024-11-05";
const SERVER_NAME: &str = "rehydration-kernel-kmp";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

const WAKE_RESPONSE_FIXTURE: &str =
    include_str!("../../../api/examples/kernel/v1beta1/kmp/wake.response.json");
const ASK_RESPONSE_FIXTURE: &str =
    include_str!("../../../api/examples/kernel/v1beta1/kmp/ask.response.json");
const TRACE_RESPONSE_FIXTURE: &str =
    include_str!("../../../api/examples/kernel/v1beta1/kmp/trace.response.json");
const INSPECT_RESPONSE_FIXTURE: &str =
    include_str!("../../../api/examples/kernel/v1beta1/kmp/inspect.response.json");

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KernelMcpBackend {
    Fixture,
    Grpc {
        endpoint: String,
        tls: KernelMcpGrpcTlsConfig,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KernelMcpGrpcTlsConfig {
    mode: KernelMcpGrpcTlsMode,
    ca_path: Option<PathBuf>,
    cert_path: Option<PathBuf>,
    key_path: Option<PathBuf>,
    domain_name: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KernelMcpGrpcTlsMode {
    Disabled,
    Server,
    Mutual,
}

impl KernelMcpGrpcTlsMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Server => "server",
            Self::Mutual => "mutual",
        }
    }
}

impl KernelMcpGrpcTlsConfig {
    pub fn disabled() -> Self {
        Self {
            mode: KernelMcpGrpcTlsMode::Disabled,
            ca_path: None,
            cert_path: None,
            key_path: None,
            domain_name: None,
        }
    }

    pub fn server(ca_path: impl Into<PathBuf>, domain_name: Option<String>) -> Self {
        Self {
            mode: KernelMcpGrpcTlsMode::Server,
            ca_path: Some(ca_path.into()),
            cert_path: None,
            key_path: None,
            domain_name,
        }
    }

    pub fn mutual(
        ca_path: impl Into<PathBuf>,
        cert_path: impl Into<PathBuf>,
        key_path: impl Into<PathBuf>,
        domain_name: Option<String>,
    ) -> Self {
        Self {
            mode: KernelMcpGrpcTlsMode::Mutual,
            ca_path: Some(ca_path.into()),
            cert_path: Some(cert_path.into()),
            key_path: Some(key_path.into()),
            domain_name,
        }
    }

    pub fn from_env_for_endpoint(endpoint: Option<&str>) -> Self {
        let ca_path = optional_env_path(GRPC_TLS_CA_PATH_ENV);
        let cert_path = optional_env_path(GRPC_TLS_CERT_PATH_ENV);
        let key_path = optional_env_path(GRPC_TLS_KEY_PATH_ENV);
        let domain_name = optional_env_string(GRPC_TLS_DOMAIN_NAME_ENV);
        let server_tls_requested = ca_path.is_some()
            || domain_name.is_some()
            || endpoint
                .map(|endpoint| endpoint.trim().starts_with("https://"))
                .unwrap_or(false);
        let mode = optional_env_string(GRPC_TLS_MODE_ENV)
            .and_then(|value| parse_tls_mode(&value))
            .unwrap_or_else(|| {
                if cert_path.is_some() || key_path.is_some() {
                    KernelMcpGrpcTlsMode::Mutual
                } else if server_tls_requested {
                    KernelMcpGrpcTlsMode::Server
                } else {
                    KernelMcpGrpcTlsMode::Disabled
                }
            });

        Self {
            mode,
            ca_path,
            cert_path,
            key_path,
            domain_name,
        }
    }

    pub fn from_env() -> Self {
        Self::from_env_for_endpoint(std::env::var(GRPC_ENDPOINT_ENV).ok().as_deref())
    }

    pub fn mode(&self) -> KernelMcpGrpcTlsMode {
        self.mode
    }

    pub fn mode_name(&self) -> &'static str {
        self.mode.as_str()
    }
}

pub struct KernelMcpServer {
    backend: KernelMcpBackend,
}

impl Default for KernelMcpServer {
    fn default() -> Self {
        Self::fixture()
    }
}

impl KernelMcpServer {
    pub fn fixture() -> Self {
        Self {
            backend: KernelMcpBackend::Fixture,
        }
    }

    pub fn grpc(endpoint: impl Into<String>) -> Self {
        Self::grpc_with_tls(endpoint, KernelMcpGrpcTlsConfig::disabled())
    }

    pub fn grpc_with_tls(endpoint: impl Into<String>, tls: KernelMcpGrpcTlsConfig) -> Self {
        Self {
            backend: KernelMcpBackend::Grpc {
                endpoint: endpoint.into(),
                tls,
            },
        }
    }

    pub fn from_env() -> Self {
        let endpoint = std::env::var(GRPC_ENDPOINT_ENV).ok();
        let tls = KernelMcpGrpcTlsConfig::from_env_for_endpoint(endpoint.as_deref());
        Self::from_optional_endpoint_and_tls(endpoint, tls)
    }

    pub fn from_optional_endpoint(endpoint: Option<String>) -> Self {
        Self::from_optional_endpoint_and_tls(endpoint, KernelMcpGrpcTlsConfig::disabled())
    }

    pub fn from_optional_endpoint_and_tls(
        endpoint: Option<String>,
        tls: KernelMcpGrpcTlsConfig,
    ) -> Self {
        match endpoint.filter(|endpoint| !endpoint.trim().is_empty()) {
            Some(endpoint) => Self::grpc_with_tls(endpoint, tls),
            None => Self::fixture(),
        }
    }

    pub fn backend_name(&self) -> &'static str {
        match self.backend {
            KernelMcpBackend::Fixture => "fixture",
            KernelMcpBackend::Grpc { .. } => "grpc",
        }
    }

    pub fn grpc_tls_mode_name(&self) -> &'static str {
        match &self.backend {
            KernelMcpBackend::Fixture => "disabled",
            KernelMcpBackend::Grpc { tls, .. } => tls.mode_name(),
        }
    }

    pub async fn handle_json_line(&self, line: &str) -> Option<String> {
        let request = match serde_json::from_str::<Value>(line) {
            Ok(request) => request,
            Err(error) => {
                return Some(jsonrpc_error(
                    Value::Null,
                    -32700,
                    &format!("invalid JSON-RPC message: {error}"),
                ));
            }
        };

        let id = request.get("id").cloned();
        let method = request.get("method").and_then(Value::as_str);

        match method {
            Some("initialize") => id.map(|id| jsonrpc_result(id, initialize_result(self))),
            Some("notifications/initialized") => None,
            Some("tools/list") => id.map(|id| jsonrpc_result(id, tools_list_result())),
            Some("tools/call") => match id {
                Some(id) => Some(self.handle_tool_call(id, request.get("params")).await),
                None => None,
            },
            Some(other) => id.map(|id| {
                jsonrpc_error(
                    id,
                    -32601,
                    &format!("unsupported JSON-RPC method `{other}`"),
                )
            }),
            None => Some(jsonrpc_error(
                Value::Null,
                -32600,
                "missing JSON-RPC method",
            )),
        }
    }

    async fn handle_tool_call(&self, id: Value, params: Option<&Value>) -> String {
        let Some(params) = params.and_then(Value::as_object) else {
            return jsonrpc_error(id, -32602, "tools/call requires object params");
        };
        let Some(name) = params.get("name").and_then(Value::as_str) else {
            return jsonrpc_error(id, -32602, "tools/call requires params.name");
        };
        let arguments = params.get("arguments").unwrap_or(&Value::Null);

        let result = match &self.backend {
            KernelMcpBackend::Fixture => fixture_tool_result(name, arguments),
            KernelMcpBackend::Grpc { endpoint, tls } => {
                grpc_tool_result(endpoint, tls, name, arguments).await
            }
        };

        match result {
            Ok(result) => jsonrpc_result(id, result),
            Err(message) => jsonrpc_result(id, tool_error_result(&message)),
        }
    }
}

fn initialize_result(server: &KernelMcpServer) -> Value {
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": {
            "tools": {}
        },
        "serverInfo": {
            "name": SERVER_NAME,
            "version": SERVER_VERSION
        },
        "metadata": {
            "backend": server.backend_name(),
            "grpc_tls": server.grpc_tls_mode_name()
        }
    })
}

fn tools_list_result() -> Value {
    json!({
        "tools": [
            tool_definition(
                "kernel_wake",
                "Return a compact Kernel Memory Protocol wake packet for continuing work from memory.",
                json!({
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["about"],
                    "properties": {
                        "about": string_schema("Memory anchor or root ref to wake from."),
                        "role": string_schema("Optional caller role."),
                        "intent": string_schema("Optional continuation intent."),
                        "depth": integer_schema("Optional graph traversal depth for live gRPC mode."),
                        "budget": budget_schema()
                    }
                })
            ),
            tool_definition(
                "kernel_ask",
                "Answer a question from kernel memory with proof, or return unknown/conflict.",
                json!({
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["about", "question"],
                    "properties": {
                        "about": string_schema("Memory anchor or root ref to ask from."),
                        "question": string_schema("Natural-language question."),
                        "answer_policy": {
                            "type": "string",
                            "enum": ["evidence_or_unknown", "show_conflicts", "best_effort"]
                        },
                        "prefer": {
                            "type": "object",
                            "additionalProperties": true
                        },
                        "depth": integer_schema("Optional graph traversal depth for live gRPC mode."),
                        "budget": budget_schema()
                    }
                })
            ),
            tool_definition(
                "kernel_trace",
                "Trace the proof path between two memory refs.",
                json!({
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["from", "to"],
                    "properties": {
                        "from": string_schema("Source memory ref. In live gRPC mode this must resolve to a kernel node id."),
                        "to": string_schema("Target memory ref. In live gRPC mode this must resolve to a kernel node id."),
                        "role": string_schema("Optional caller role."),
                        "goal": string_schema("Optional trace goal."),
                        "budget": budget_schema(),
                        "include": {
                            "type": "object",
                            "additionalProperties": false,
                            "properties": {
                                "evidence": {"type": "boolean"},
                                "raw_refs": {"type": "boolean"}
                            }
                        }
                    }
                })
            ),
            tool_definition(
                "kernel_inspect",
                "Inspect the raw stored memory object, links, and evidence for one ref.",
                json!({
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["ref"],
                    "properties": {
                        "ref": string_schema("Memory ref to inspect. In live gRPC mode this must resolve to a kernel node id."),
                        "include": {
                            "type": "object",
                            "additionalProperties": false,
                            "properties": {
                                "incoming": {"type": "boolean"},
                                "outgoing": {"type": "boolean"},
                                "details": {"type": "boolean"},
                                "raw": {"type": "boolean"}
                            }
                        }
                    }
                })
            )
        ]
    })
}

fn tool_definition(name: &str, description: &str, input_schema: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema
    })
}

fn string_schema(description: &str) -> Value {
    json!({
        "type": "string",
        "minLength": 1,
        "description": description
    })
}

fn integer_schema(description: &str) -> Value {
    json!({
        "type": "integer",
        "minimum": 1,
        "description": description
    })
}

fn budget_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "tokens": {
                "type": "integer",
                "minimum": 1
            },
            "detail": {
                "type": "string",
                "enum": ["compact", "balanced", "full"]
            }
        }
    })
}

fn fixture_tool_result(name: &str, arguments: &Value) -> Result<Value, String> {
    match name {
        "kernel_wake" => read_fixture_tool_result(arguments, &["about"], WAKE_RESPONSE_FIXTURE),
        "kernel_ask" => {
            read_fixture_tool_result(arguments, &["about", "question"], ASK_RESPONSE_FIXTURE)
        }
        "kernel_trace" => {
            read_fixture_tool_result(arguments, &["from", "to"], TRACE_RESPONSE_FIXTURE)
        }
        "kernel_inspect" => read_fixture_tool_result(arguments, &["ref"], INSPECT_RESPONSE_FIXTURE),
        "kernel_remember" => {
            Err("kernel_remember is not implemented in the read-only MCP adapter".to_string())
        }
        other => Err(format!("unknown KMP tool `{other}`")),
    }
}

fn read_fixture_tool_result(
    arguments: &Value,
    required_arguments: &[&str],
    fixture: &str,
) -> Result<Value, String> {
    validate_required_arguments(arguments, required_arguments)?;
    let structured_content = serde_json::from_str::<Value>(fixture)
        .map_err(|error| format!("fixture response is invalid JSON: {error}"))?;
    Ok(tool_success_result(structured_content))
}

async fn grpc_tool_result(
    endpoint: &str,
    tls: &KernelMcpGrpcTlsConfig,
    name: &str,
    arguments: &Value,
) -> Result<Value, String> {
    match name {
        "kernel_wake" => grpc_wake(endpoint, tls, arguments).await,
        "kernel_ask" => grpc_ask(endpoint, tls, arguments).await,
        "kernel_trace" => grpc_trace(endpoint, tls, arguments).await,
        "kernel_inspect" => grpc_inspect(endpoint, tls, arguments).await,
        "kernel_remember" => {
            Err("kernel_remember is not implemented in the read-only MCP adapter".to_string())
        }
        other => Err(format!("unknown KMP tool `{other}`")),
    }
}

async fn grpc_wake(
    endpoint: &str,
    tls: &KernelMcpGrpcTlsConfig,
    arguments: &Value,
) -> Result<Value, String> {
    validate_required_arguments(arguments, &["about"])?;
    let mut client = connect_query_client(endpoint, tls).await?;
    let about = required_string(arguments, "about")?;
    let role = optional_string(arguments, "role").unwrap_or_else(|| "agent".to_string());
    let intent = optional_string(arguments, "intent")
        .unwrap_or_else(|| format!("continue from live kernel context `{about}`"));
    let token_budget = budget_tokens(arguments).unwrap_or(1600);
    let depth = optional_u32(arguments, "depth").unwrap_or(2);

    let response = client
        .get_context(GetContextRequest {
            root_node_id: about.clone(),
            role,
            token_budget,
            requested_scopes: Vec::new(),
            depth,
            max_tier: ResolutionTier::L2EvidencePack as i32,
            rehydration_mode: RehydrationMode::ResumeFocused as i32,
        })
        .await
        .map_err(|status| format!("GetContext failed for `{about}`: {status}"))?
        .into_inner();

    Ok(tool_success_result(wake_from_get_context(
        &about, &intent, &response,
    )))
}

async fn grpc_ask(
    endpoint: &str,
    tls: &KernelMcpGrpcTlsConfig,
    arguments: &Value,
) -> Result<Value, String> {
    validate_required_arguments(arguments, &["about", "question"])?;
    let mut client = connect_query_client(endpoint, tls).await?;
    let about = required_string(arguments, "about")?;
    let question = required_string(arguments, "question")?;
    let token_budget = budget_tokens(arguments).unwrap_or(2400);
    let depth = optional_u32(arguments, "depth").unwrap_or(2);

    let response = client
        .get_context(GetContextRequest {
            root_node_id: about.clone(),
            role: "answerer".to_string(),
            token_budget,
            requested_scopes: Vec::new(),
            depth,
            max_tier: ResolutionTier::L2EvidencePack as i32,
            rehydration_mode: RehydrationMode::ReasonPreserving as i32,
        })
        .await
        .map_err(|status| format!("GetContext failed for `{about}`: {status}"))?
        .into_inner();

    Ok(tool_success_result(ask_from_get_context(
        &about, &question, &response,
    )))
}

async fn grpc_trace(
    endpoint: &str,
    tls: &KernelMcpGrpcTlsConfig,
    arguments: &Value,
) -> Result<Value, String> {
    validate_required_arguments(arguments, &["from", "to"])?;
    let mut client = connect_query_client(endpoint, tls).await?;
    let from = required_string(arguments, "from")?;
    let to = required_string(arguments, "to")?;
    let role = optional_string(arguments, "role").unwrap_or_else(|| "tracer".to_string());
    let token_budget = budget_tokens(arguments).unwrap_or(1600);

    let response = client
        .get_context_path(GetContextPathRequest {
            root_node_id: from.clone(),
            target_node_id: to.clone(),
            role,
            token_budget,
        })
        .await
        .map_err(|status| format!("GetContextPath failed for `{from}` -> `{to}`: {status}"))?
        .into_inner();

    let relationships = response
        .path_bundle
        .as_ref()
        .map(bundle_relationships)
        .unwrap_or_default();
    let rendered_summary = response
        .rendered
        .as_ref()
        .map(rendered_summary)
        .unwrap_or_else(|| format!("Traced live kernel path from {from} to {to}."));

    Ok(tool_success_result(json!({
        "summary": rendered_summary,
        "trace": relationships,
        "warnings": live_warnings(response.rendered.as_ref(), relationships_is_empty(&relationships))
    })))
}

async fn grpc_inspect(
    endpoint: &str,
    tls: &KernelMcpGrpcTlsConfig,
    arguments: &Value,
) -> Result<Value, String> {
    validate_required_arguments(arguments, &["ref"])?;
    let mut client = connect_query_client(endpoint, tls).await?;
    let ref_id = required_string(arguments, "ref")?;

    let response = client
        .get_node_detail(GetNodeDetailRequest {
            node_id: ref_id.clone(),
        })
        .await
        .map_err(|status| format!("GetNodeDetail failed for `{ref_id}`: {status}"))?
        .into_inner();

    Ok(tool_success_result(inspect_from_get_node_detail(
        &ref_id, &response,
    )))
}

async fn connect_query_client(
    endpoint: &str,
    tls: &KernelMcpGrpcTlsConfig,
) -> Result<ContextQueryServiceClient<tonic::transport::Channel>, String> {
    let endpoint_uri = endpoint_uri_for_tls_mode(endpoint, tls.mode);
    let mut endpoint = Endpoint::from_shared(endpoint_uri.clone()).map_err(|error| {
        format!("invalid kernel gRPC endpoint `{endpoint_uri}` from {GRPC_ENDPOINT_ENV}: {error}")
    })?;

    if tls.mode != KernelMcpGrpcTlsMode::Disabled {
        endpoint = endpoint.tls_config(client_tls_config(tls)?).map_err(|error| {
            format!(
                "invalid kernel gRPC TLS config from {GRPC_TLS_MODE_ENV}/{GRPC_TLS_CA_PATH_ENV}/{GRPC_TLS_CERT_PATH_ENV}/{GRPC_TLS_KEY_PATH_ENV}: {error}"
            )
        })?;
    }

    endpoint
        .connect()
        .await
        .map_err(|error| {
            format!(
                "failed to connect to kernel gRPC endpoint `{endpoint_uri}` from {GRPC_ENDPOINT_ENV} with TLS mode `{}`: {error}; debug={error:?}",
                tls.mode_name()
            )
        })
        .map(ContextQueryServiceClient::new)
}

fn client_tls_config(tls: &KernelMcpGrpcTlsConfig) -> Result<ClientTlsConfig, String> {
    install_rustls_crypto_provider();

    let mut config = ClientTlsConfig::new().with_enabled_roots();

    if let Some(ca_path) = tls.ca_path.as_ref() {
        let ca_pem = fs::read(ca_path).map_err(|error| {
            format!(
                "failed to read {GRPC_TLS_CA_PATH_ENV} `{}`: {error}",
                ca_path.display()
            )
        })?;
        config = config.ca_certificate(Certificate::from_pem(ca_pem));
    }

    if let Some(domain_name) = tls.domain_name.as_deref() {
        config = config.domain_name(domain_name.to_string());
    }

    if tls.mode == KernelMcpGrpcTlsMode::Mutual {
        let cert_path = tls.cert_path.as_ref().ok_or_else(|| {
            format!("{GRPC_TLS_CERT_PATH_ENV} is required when {GRPC_TLS_MODE_ENV}=mutual")
        })?;
        let key_path = tls.key_path.as_ref().ok_or_else(|| {
            format!("{GRPC_TLS_KEY_PATH_ENV} is required when {GRPC_TLS_MODE_ENV}=mutual")
        })?;
        let cert_pem = fs::read(cert_path).map_err(|error| {
            format!(
                "failed to read {GRPC_TLS_CERT_PATH_ENV} `{}`: {error}",
                cert_path.display()
            )
        })?;
        let key_pem = fs::read(key_path).map_err(|error| {
            format!(
                "failed to read {GRPC_TLS_KEY_PATH_ENV} `{}`: {error}",
                key_path.display()
            )
        })?;
        config = config.identity(Identity::from_pem(cert_pem, key_pem));
    }

    Ok(config)
}

fn endpoint_uri_for_tls_mode(endpoint: &str, mode: KernelMcpGrpcTlsMode) -> String {
    if mode == KernelMcpGrpcTlsMode::Disabled {
        return endpoint.to_string();
    }

    endpoint
        .strip_prefix("http://")
        .map(|without_scheme| format!("https://{without_scheme}"))
        .unwrap_or_else(|| endpoint.to_string())
}

fn parse_tls_mode(value: &str) -> Option<KernelMcpGrpcTlsMode> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" => None,
        "disabled" | "disable" | "off" | "false" | "none" => Some(KernelMcpGrpcTlsMode::Disabled),
        "server" | "tls" => Some(KernelMcpGrpcTlsMode::Server),
        "mutual" | "mtls" | "m-tls" => Some(KernelMcpGrpcTlsMode::Mutual),
        _ => None,
    }
}

fn optional_env_path(name: &str) -> Option<PathBuf> {
    optional_env_string(name).map(PathBuf::from)
}

fn optional_env_string(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn install_rustls_crypto_provider() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let _ = tokio_rustls::rustls::crypto::aws_lc_rs::default_provider().install_default();
    });
}

fn wake_from_get_context(about: &str, intent: &str, response: &GetContextResponse) -> Value {
    let rendered = response.rendered.as_ref();
    let relationships = context_relationships(response);
    let evidence = context_evidence(response);
    let current_state = rendered_current_state(rendered);
    let has_content = rendered
        .map(|rendered| !rendered.content.trim().is_empty() || !rendered.sections.is_empty())
        .unwrap_or(false);

    json!({
        "summary": rendered
            .map(rendered_summary)
            .unwrap_or_else(|| format!("Live kernel returned no rendered context for {about}.")),
        "wake": {
            "objective": intent,
            "current_state": current_state,
            "causal_spine": relationships
                .iter()
                .take(8)
                .map(|relationship| json!({
                    "claim": format!(
                        "{} -> {}",
                        relationship.get("from").and_then(Value::as_str).unwrap_or("unknown"),
                        relationship.get("to").and_then(Value::as_str).unwrap_or("unknown")
                    ),
                    "because": relationship
                        .get("why")
                        .and_then(Value::as_str)
                        .filter(|why| !why.is_empty())
                        .unwrap_or("Kernel relationship path selected this edge."),
                    "evidence_ref": relationship
                        .get("evidence")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                }))
                .collect::<Vec<_>>(),
            "open_loops": if has_content { Vec::<String>::new() } else { vec!["No rendered live context was returned.".to_string()] },
            "next_actions": [
                "Use kernel_trace for specific relation paths.",
                "Use kernel_inspect for raw node detail."
            ],
            "guardrails": [
                "This wake packet is derived from live GetContext output.",
                "Missing relations or details may limit proof quality."
            ]
        },
        "proof": {
            "path": relationships,
            "evidence": evidence,
            "conflicts": [],
            "missing": if has_content { Vec::<String>::new() } else { vec!["rendered_context".to_string()] },
            "confidence": if has_content { "medium" } else { "unknown" }
        },
        "warnings": live_warnings(rendered, false)
    })
}

fn ask_from_get_context(about: &str, question: &str, response: &GetContextResponse) -> Value {
    let rendered = response.rendered.as_ref();
    let relationships = context_relationships(response);
    let evidence = context_evidence(response);
    let has_evidence = !evidence.is_empty()
        || rendered
            .map(|rendered| !rendered.content.trim().is_empty())
            .unwrap_or(false);

    json!({
        "summary": if has_evidence {
            format!("Returned live kernel context for `{about}`. This read-only adapter did not generate a final answer for: {question}")
        } else {
            format!("Live kernel returned no evidence for `{about}`.")
        },
        "answer": Value::Null,
        "because": evidence
            .iter()
            .take(5)
            .map(|item| json!({
                "claim": item.get("source").and_then(Value::as_str).unwrap_or("kernel evidence"),
                "evidence": item.get("text").and_then(Value::as_str).unwrap_or(""),
                "ref": item.get("id").and_then(Value::as_str).unwrap_or("")
            }))
            .collect::<Vec<_>>(),
        "proof": {
            "path": relationships,
            "evidence": evidence,
            "conflicts": [],
            "missing": ["generative_answer"],
            "confidence": if has_evidence { "medium" } else { "unknown" }
        },
        "warnings": [
            "kernel_ask live gRPC mode returns evidence/proof only; final answer generation is not implemented in this adapter."
        ]
    })
}

fn inspect_from_get_node_detail(ref_id: &str, response: &GetNodeDetailResponse) -> Value {
    let object = response.node.as_ref().map_or_else(
        || {
            json!({
                "ref": ref_id,
                "kind": "unknown"
            })
        },
        |node| {
            json!({
                "ref": node.node_id,
                "kind": node.node_kind,
                "text": if node.summary.is_empty() { node.title.clone() } else { node.summary.clone() }
            })
        },
    );
    let evidence = response
        .detail
        .as_ref()
        .map_or_else(Vec::new, |detail| vec![evidence_from_detail(detail)]);

    json!({
        "summary": if response.node.is_some() {
            format!("Found live kernel node `{ref_id}`.")
        } else {
            format!("No live kernel node metadata returned for `{ref_id}`.")
        },
        "object": object,
        "links": {
            "incoming": [],
            "outgoing": []
        },
        "evidence": evidence,
        "warnings": if response.detail.is_some() { Vec::<String>::new() } else { vec!["No node detail returned.".to_string()] }
    })
}

fn context_relationships(response: &GetContextResponse) -> Vec<Value> {
    response
        .bundle
        .as_ref()
        .map(bundle_relationships)
        .unwrap_or_default()
}

fn bundle_relationships(bundle: &rehydration_proto::v1beta1::RehydrationBundle) -> Vec<Value> {
    bundle
        .bundles
        .iter()
        .flat_map(|role_bundle| role_bundle.relationships.iter())
        .map(relationship_json)
        .collect()
}

fn relationships_is_empty(relationships: &[Value]) -> bool {
    relationships.is_empty()
}

fn relationship_json(relationship: &GraphRelationship) -> Value {
    let explanation = relationship.explanation.as_ref();
    let relationship_type = if relationship.relationship_type.trim().is_empty() {
        "related"
    } else {
        relationship.relationship_type.as_str()
    };
    let why = explanation
        .map(|explanation| {
            first_non_empty([
                explanation.rationale.as_str(),
                explanation.motivation.as_str(),
                explanation.method.as_str(),
            ])
        })
        .filter(|why| !why.trim().is_empty())
        .unwrap_or_else(|| "Kernel relationship path selected this edge.".to_string());
    let evidence = explanation
        .map(|explanation| explanation.evidence.clone())
        .filter(|evidence| !evidence.trim().is_empty())
        .unwrap_or_else(|| why.clone());

    json!({
        "from": relationship.source_node_id,
        "to": relationship.target_node_id,
        "rel": relationship_type,
        "class": explanation
            .map(|explanation| semantic_class_label(explanation.semantic_class))
            .unwrap_or("structural"),
        "why": why,
        "evidence": evidence,
        "confidence": explanation
            .map(|explanation| if explanation.confidence.is_empty() { "unknown".to_string() } else { explanation.confidence.clone() })
            .unwrap_or_else(|| "unknown".to_string())
    })
}

fn semantic_class_label(value: i32) -> &'static str {
    match GraphRelationshipSemanticClass::try_from(value) {
        Ok(GraphRelationshipSemanticClass::Structural) => "structural",
        Ok(GraphRelationshipSemanticClass::Causal) => "causal",
        Ok(GraphRelationshipSemanticClass::Motivational) => "motivational",
        Ok(GraphRelationshipSemanticClass::Procedural) => "procedural",
        Ok(GraphRelationshipSemanticClass::Evidential) => "evidential",
        Ok(GraphRelationshipSemanticClass::Constraint) => "constraint",
        _ => "structural",
    }
}

fn first_non_empty(values: [&str; 3]) -> String {
    values
        .into_iter()
        .find(|value| !value.trim().is_empty())
        .unwrap_or("")
        .to_string()
}

fn context_evidence(response: &GetContextResponse) -> Vec<Value> {
    response
        .bundle
        .as_ref()
        .map(|bundle| {
            bundle
                .bundles
                .iter()
                .flat_map(|role_bundle| role_bundle.node_details.iter())
                .map(evidence_from_detail)
                .collect()
        })
        .unwrap_or_default()
}

fn evidence_from_detail(detail: &BundleNodeDetail) -> Value {
    json!({
        "id": format!("detail:{}", detail.node_id),
        "supports": [detail.node_id.clone()],
        "text": detail.detail,
        "source": detail.node_id
    })
}

fn rendered_current_state(rendered: Option<&RenderedContext>) -> Vec<String> {
    let Some(rendered) = rendered else {
        return Vec::new();
    };

    let from_sections = rendered
        .sections
        .iter()
        .take(5)
        .map(|section| {
            if section.title.is_empty() {
                section.content.clone()
            } else {
                format!("{}: {}", section.title, section.content)
            }
        })
        .filter(|state| !state.trim().is_empty())
        .collect::<Vec<_>>();

    if !from_sections.is_empty() {
        return from_sections;
    }

    if rendered.content.trim().is_empty() {
        Vec::new()
    } else {
        vec![truncate(&rendered.content, 1200)]
    }
}

fn rendered_summary(rendered: &RenderedContext) -> String {
    rendered
        .tiers
        .iter()
        .find(|tier| !tier.content.trim().is_empty())
        .map(|tier| truncate(&tier.content, 500))
        .or_else(|| {
            rendered
                .sections
                .iter()
                .find(|section| !section.content.trim().is_empty())
                .map(|section| truncate(&section.content, 500))
        })
        .unwrap_or_else(|| truncate(&rendered.content, 500))
}

fn live_warnings(rendered: Option<&RenderedContext>, missing_path: bool) -> Vec<String> {
    let mut warnings = Vec::new();

    if rendered
        .map(|rendered| rendered.content.trim().is_empty() && rendered.sections.is_empty())
        .unwrap_or(true)
    {
        warnings.push("No rendered context was returned by the live kernel.".to_string());
    }

    if missing_path {
        warnings.push("No relationship path was returned by the live kernel.".to_string());
    }

    warnings
}

fn truncate(text: &str, max_chars: usize) -> String {
    let mut truncated = text.chars().take(max_chars).collect::<String>();
    if text.chars().count() > max_chars {
        truncated.push_str("...");
    }
    truncated
}

fn validate_required_arguments(
    arguments: &Value,
    required_arguments: &[&str],
) -> Result<(), String> {
    let Some(arguments) = arguments.as_object() else {
        return Err("tool arguments must be a JSON object".to_string());
    };

    for required_argument in required_arguments {
        let present = arguments
            .get(*required_argument)
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty());

        if !present {
            return Err(format!("missing required argument `{required_argument}`"));
        }
    }

    Ok(())
}

fn required_string(arguments: &Value, key: &str) -> Result<String, String> {
    arguments
        .as_object()
        .and_then(|arguments| arguments.get(key))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| format!("missing required argument `{key}`"))
}

fn optional_string(arguments: &Value, key: &str) -> Option<String> {
    arguments
        .as_object()
        .and_then(|arguments| arguments.get(key))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
}

fn optional_u32(arguments: &Value, key: &str) -> Option<u32> {
    arguments
        .as_object()
        .and_then(|arguments| arguments.get(key))
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
}

fn budget_tokens(arguments: &Value) -> Option<u32> {
    arguments
        .as_object()
        .and_then(|arguments| arguments.get("budget"))
        .and_then(Value::as_object)
        .and_then(|budget| budget.get("tokens"))
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
}

fn tool_success_result(structured_content: Value) -> Value {
    let text = serde_json::to_string_pretty(&structured_content)
        .expect("fixture JSON should serialize as pretty text");
    json!({
        "content": [
            {
                "type": "text",
                "text": text
            }
        ],
        "structuredContent": structured_content,
        "isError": false
    })
}

fn tool_error_result(message: &str) -> Value {
    json!({
        "content": [
            {
                "type": "text",
                "text": message
            }
        ],
        "isError": true
    })
}

fn jsonrpc_result(id: Value, result: Value) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
    .to_string()
}

fn jsonrpc_error(id: Value, code: i64, message: &str) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    })
    .to_string()
}
