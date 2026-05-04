mod channel;
mod requests;

use serde_json::Value;

use crate::backend::{KernelMcpGrpcTlsConfig, KernelMcpToolBackend, KernelMcpToolFuture};
use crate::grpc::channel::connect_memory_client;
use crate::grpc::requests::{
    ask_request_from_arguments, ingest_request_from_arguments, inspect_request_from_arguments,
    temporal_move_request_from_arguments, temporal_near_request_from_arguments,
    trace_request_from_arguments, wake_request_from_arguments,
};
use crate::ingest::build_ingest_plan;
use crate::kmp::{
    ask_from_response, dry_run_ingest_from_plan, ingest_from_response, inspect_from_response,
    temporal_from_response, trace_from_response, wake_from_response,
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
        "kernel_goto" => grpc_temporal_move(endpoint, tls, "goto", arguments).await,
        "kernel_near" => grpc_temporal_near(endpoint, tls, arguments).await,
        "kernel_rewind" => grpc_temporal_move(endpoint, tls, "rewind", arguments).await,
        "kernel_forward" => grpc_temporal_move(endpoint, tls, "forward", arguments).await,
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
    let request = ingest_request_from_arguments(arguments)?;
    if request.dry_run {
        return Ok(tool_success_result(dry_run_ingest_from_plan(&plan)));
    }

    let about = request.about.clone();
    let mut client = connect_memory_client(endpoint, tls).await?;
    let response = client
        .ingest(request)
        .await
        .map_err(|status| format!("KernelMemoryService.Ingest failed for `{about}`: {status}"))?
        .into_inner();

    Ok(tool_success_result(ingest_from_response(response)))
}

async fn grpc_wake(
    endpoint: &str,
    tls: &KernelMcpGrpcTlsConfig,
    arguments: &Value,
) -> Result<Value, String> {
    let request = wake_request_from_arguments(arguments)?;
    let about = request.about.clone();
    let mut client = connect_memory_client(endpoint, tls).await?;
    let response = client
        .wake(request)
        .await
        .map_err(|status| format!("KernelMemoryService.Wake failed for `{about}`: {status}"))?
        .into_inner();

    Ok(tool_success_result(wake_from_response(response)))
}

async fn grpc_ask(
    endpoint: &str,
    tls: &KernelMcpGrpcTlsConfig,
    arguments: &Value,
) -> Result<Value, String> {
    let request = ask_request_from_arguments(arguments)?;
    let about = request.about.clone();
    let mut client = connect_memory_client(endpoint, tls).await?;
    let response = client
        .ask(request)
        .await
        .map_err(|status| format!("KernelMemoryService.Ask failed for `{about}`: {status}"))?
        .into_inner();

    Ok(tool_success_result(ask_from_response(response)))
}

async fn grpc_temporal_move(
    endpoint: &str,
    tls: &KernelMcpGrpcTlsConfig,
    direction: &str,
    arguments: &Value,
) -> Result<Value, String> {
    let request = temporal_move_request_from_arguments(arguments, direction)?;
    let about = request.about.clone();
    let mut client = connect_memory_client(endpoint, tls).await?;
    let response = match direction {
        "goto" => client.goto(request).await,
        "rewind" => client.rewind(request).await,
        "forward" => client.forward(request).await,
        _ => return Err(format!("unknown temporal direction `{direction}`")),
    }
    .map_err(|status| {
        format!(
            "KernelMemoryService.{} failed for `{about}`: {status}",
            method_name(direction)
        )
    })?
    .into_inner();

    Ok(tool_success_result(temporal_from_response(response)))
}

async fn grpc_temporal_near(
    endpoint: &str,
    tls: &KernelMcpGrpcTlsConfig,
    arguments: &Value,
) -> Result<Value, String> {
    let request = temporal_near_request_from_arguments(arguments)?;
    let about = request.about.clone();
    let mut client = connect_memory_client(endpoint, tls).await?;
    let response = client
        .near(request)
        .await
        .map_err(|status| format!("KernelMemoryService.Near failed for `{about}`: {status}"))?
        .into_inner();

    Ok(tool_success_result(temporal_from_response(response)))
}

async fn grpc_trace(
    endpoint: &str,
    tls: &KernelMcpGrpcTlsConfig,
    arguments: &Value,
) -> Result<Value, String> {
    let request = trace_request_from_arguments(arguments)?;
    let from = request.from.clone();
    let to = request.to.clone();
    let mut client = connect_memory_client(endpoint, tls).await?;
    let response = client
        .trace(request)
        .await
        .map_err(|status| {
            format!("KernelMemoryService.Trace failed for `{from}` -> `{to}`: {status}")
        })?
        .into_inner();

    Ok(tool_success_result(trace_from_response(response)))
}

async fn grpc_inspect(
    endpoint: &str,
    tls: &KernelMcpGrpcTlsConfig,
    arguments: &Value,
) -> Result<Value, String> {
    let request = inspect_request_from_arguments(arguments)?;
    let ref_id = request.r#ref.clone();
    let mut client = connect_memory_client(endpoint, tls).await?;
    let response = client
        .inspect(request)
        .await
        .map_err(|status| format!("KernelMemoryService.Inspect failed for `{ref_id}`: {status}"))?
        .into_inner();

    Ok(tool_success_result(inspect_from_response(response)))
}

fn method_name(direction: &str) -> &'static str {
    match direction {
        "goto" => "Goto",
        "rewind" => "Rewind",
        "forward" => "Forward",
        _ => "TemporalMove",
    }
}
