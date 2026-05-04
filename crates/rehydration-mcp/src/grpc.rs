use std::fs;
use std::sync::Once;

use rehydration_proto::v1beta1::{
    CommandMetadata, ContextChange, ContextChangeOperation, GetContextPathRequest,
    GetContextRequest, GetNodeDetailRequest, RehydrationMode, ResolutionTier, UpdateContextRequest,
    context_command_service_client::ContextCommandServiceClient,
    context_query_service_client::ContextQueryServiceClient,
};
use serde_json::{Value, json};
use tonic::transport::{Certificate, Channel, ClientTlsConfig, Endpoint, Identity};

use crate::args::{
    budget_tokens, optional_string, optional_u32, required_string, validate_required_arguments,
};
use crate::backend::{
    GRPC_ENDPOINT_ENV, GRPC_TLS_CA_PATH_ENV, GRPC_TLS_CERT_PATH_ENV, GRPC_TLS_KEY_PATH_ENV,
    GRPC_TLS_MODE_ENV, KernelMcpGrpcTlsConfig, KernelMcpGrpcTlsMode, KernelMcpToolBackend,
    KernelMcpToolFuture, endpoint_uri_for_tls_mode,
};
use crate::ingest::{build_ingest_plan, ingest_response};
use crate::kmp::{
    ask_from_get_context, bundle_relationships, inspect_from_get_node_detail, live_warnings,
    relationships_is_empty, rendered_summary, temporal_from_get_context, wake_from_get_context,
};
use crate::protocol::tool_success_result;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GrpcKernelMcpBackend {
    endpoint: String,
    tls: KernelMcpGrpcTlsConfig,
}

impl GrpcKernelMcpBackend {
    pub fn new(endpoint: impl Into<String>, tls: KernelMcpGrpcTlsConfig) -> Self {
        Self {
            endpoint: endpoint.into(),
            tls,
        }
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub fn tls(&self) -> &KernelMcpGrpcTlsConfig {
        &self.tls
    }
}

impl KernelMcpToolBackend for GrpcKernelMcpBackend {
    fn backend_name(&self) -> &'static str {
        "grpc"
    }

    fn grpc_tls_mode_name(&self) -> &'static str {
        self.tls.mode_name()
    }

    fn call_tool<'a>(&'a self, name: &'a str, arguments: &'a Value) -> KernelMcpToolFuture<'a> {
        Box::pin(async move { grpc_tool_result(&self.endpoint, &self.tls, name, arguments).await })
    }
}

async fn grpc_tool_result(
    endpoint: &str,
    tls: &KernelMcpGrpcTlsConfig,
    name: &str,
    arguments: &Value,
) -> Result<Value, String> {
    match name {
        "kernel_ingest" | "kernel_remember" | "kernel_ingest_context" => {
            grpc_ingest(endpoint, tls, arguments).await
        }
        "kernel_wake" => grpc_wake(endpoint, tls, arguments).await,
        "kernel_ask" => grpc_ask(endpoint, tls, arguments).await,
        "kernel_goto" => grpc_temporal(endpoint, tls, "goto", arguments).await,
        "kernel_near" => grpc_temporal(endpoint, tls, "near", arguments).await,
        "kernel_rewind" => grpc_temporal(endpoint, tls, "rewind", arguments).await,
        "kernel_forward" => grpc_temporal(endpoint, tls, "forward", arguments).await,
        "kernel_trace" => grpc_trace(endpoint, tls, arguments).await,
        "kernel_inspect" => grpc_inspect(endpoint, tls, arguments).await,
        other => Err(format!("unknown KMP tool `{other}`")),
    }
}

async fn grpc_ingest(
    endpoint: &str,
    tls: &KernelMcpGrpcTlsConfig,
    arguments: &Value,
) -> Result<Value, String> {
    let plan = build_ingest_plan(arguments)?;
    if plan.dry_run {
        return Ok(tool_success_result(ingest_response(
            &plan,
            Vec::new(),
            false,
        )));
    }

    let mut client = connect_command_client(endpoint, tls).await?;
    let response = client
        .update_context(UpdateContextRequest {
            root_node_id: plan.about.clone(),
            role: "memory".to_string(),
            work_item_id: plan.idempotency_key.clone(),
            changes: plan
                .changes
                .iter()
                .map(|change| ContextChange {
                    operation: ContextChangeOperation::Upsert as i32,
                    entity_kind: change.entity_kind.clone(),
                    entity_id: change.entity_id.clone(),
                    payload_json: change.payload_json.clone(),
                    reason: change.reason.clone(),
                    scopes: change.scopes.clone(),
                })
                .collect(),
            metadata: Some(CommandMetadata {
                idempotency_key: plan.idempotency_key.clone(),
                correlation_id: plan.correlation_id.clone().unwrap_or_default(),
                causation_id: plan.causation_id.clone().unwrap_or_default(),
                requested_by: plan.requested_by.clone().unwrap_or_default(),
                requested_at: None,
            }),
            precondition: None,
        })
        .await
        .map_err(|status| format!("UpdateContext failed for `{}`: {status}", plan.about))?
        .into_inner();

    Ok(tool_success_result(ingest_response(
        &plan,
        response.warnings,
        true,
    )))
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

async fn grpc_temporal(
    endpoint: &str,
    tls: &KernelMcpGrpcTlsConfig,
    direction: &str,
    arguments: &Value,
) -> Result<Value, String> {
    validate_required_arguments(arguments, &["about"])?;
    let cursor_key = match direction {
        "goto" => "at",
        "near" => "around",
        "rewind" | "forward" => "from",
        _ => return Err(format!("unknown temporal direction `{direction}`")),
    };
    if !arguments
        .get(cursor_key)
        .and_then(Value::as_object)
        .is_some_and(|cursor| {
            ["time", "sequence", "ref"]
                .iter()
                .any(|key| cursor.get(*key).is_some())
        })
    {
        return Err(format!(
            "missing required temporal cursor object `{cursor_key}` with one of `time`, `sequence`, or `ref`"
        ));
    }

    let mut client = connect_query_client(endpoint, tls).await?;
    let about = required_string(arguments, "about")?;
    let token_budget = budget_tokens(arguments).unwrap_or(2400);
    let depth = optional_u32(arguments, "depth").unwrap_or(3);

    let response = client
        .get_context(GetContextRequest {
            root_node_id: about.clone(),
            role: "temporal-reader".to_string(),
            token_budget,
            requested_scopes: Vec::new(),
            depth,
            max_tier: ResolutionTier::L2EvidencePack as i32,
            rehydration_mode: RehydrationMode::ReasonPreserving as i32,
        })
        .await
        .map_err(|status| {
            format!("GetContext failed for temporal {direction} `{about}`: {status}")
        })?
        .into_inner();

    Ok(tool_success_result(temporal_from_get_context(
        direction, arguments, &response,
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
    connect_channel(endpoint, tls)
        .await
        .map(ContextQueryServiceClient::new)
}

async fn connect_command_client(
    endpoint: &str,
    tls: &KernelMcpGrpcTlsConfig,
) -> Result<ContextCommandServiceClient<tonic::transport::Channel>, String> {
    connect_channel(endpoint, tls)
        .await
        .map(ContextCommandServiceClient::new)
}

async fn connect_channel(endpoint: &str, tls: &KernelMcpGrpcTlsConfig) -> Result<Channel, String> {
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

fn install_rustls_crypto_provider() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let _ = tokio_rustls::rustls::crypto::aws_lc_rs::default_provider().install_default();
    });
}
