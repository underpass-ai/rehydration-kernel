use std::env;
use std::error::Error;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::time::{SystemTime, UNIX_EPOCH};

use rehydration_mcp::{KernelMcpGrpcTlsConfig, KernelMcpServer};
use serde_json::{Value, json};
use underpass_operator_replay_application::{
    ReplayApplicationError, ReplayMcpPredictionsRequest, ReplayMcpPredictionsUseCase,
    ReplayProgress, ReplayProgressObserver, ReplayToolCallRequest, ReplayToolCallResponse,
    ReplayToolCaller,
};
use underpass_operator_replay_infra::{JsonlReplayReader, ReplayOutputWriter};

#[derive(Debug, Clone, PartialEq, Eq)]
struct Args {
    trajectories: PathBuf,
    predictions: PathBuf,
    output: PathBuf,
    endpoint: Option<String>,
    limit: Option<usize>,
    offset: usize,
    log_progress_every: Option<usize>,
    force: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let args = parse_args(env::args().skip(1))?;
    ReplayOutputWriter::ensure_output_dir(&args.output, args.force)?;

    let trajectories = select_trajectories(
        JsonlReplayReader::read_trajectories(&args.trajectories)?,
        &args,
    );
    let predictions = JsonlReplayReader::read_predictions(&args.predictions)?;
    let server = match args.endpoint.as_deref() {
        Some(endpoint) => KernelMcpServer::grpc_with_tls(
            endpoint,
            KernelMcpGrpcTlsConfig::from_env_for_endpoint(Some(endpoint)),
        ),
        None => KernelMcpServer::try_from_env()
            .map_err(|error| format!("failed to configure MCP gRPC backend from env: {error}"))?,
    };
    let endpoint_label = args
        .endpoint
        .clone()
        .or_else(|| env::var("REHYDRATION_KERNEL_GRPC_ENDPOINT").ok())
        .unwrap_or_else(|| "env".to_string());

    let report = ReplayMcpPredictionsUseCase::new_with_progress(
        KernelMcpToolCaller { server },
        JsonProgressLogger {
            every: args.log_progress_every,
        },
    )
    .execute(ReplayMcpPredictionsRequest {
        generated_at_unix_seconds: now_unix_seconds()?,
        endpoint: endpoint_label,
        trajectories_label: args.trajectories.display().to_string(),
        predictions_label: args.predictions.display().to_string(),
        output_label: args.output.display().to_string(),
        starting_request_id: 1,
        trajectories,
        predictions,
    })
    .await?;

    ReplayOutputWriter::write_report(&args.output, &report)?;
    println!("{}", serde_json::to_string_pretty(&report.summary)?);

    if report.summary.missing_predictions > 0
        || report.summary.invalid_predictions > 0
        || report.summary.unbounded_tool_calls > 0
        || report.summary.failed_tool_calls > 0
        || report.summary.missing_expected_ref_rows > 0
    {
        return Err(format!(
            "kernel operator MCP replay failed: missing_predictions={} invalid_predictions={} unbounded_tool_calls={} failed_tool_calls={} missing_expected_ref_rows={}",
            report.summary.missing_predictions,
            report.summary.invalid_predictions,
            report.summary.unbounded_tool_calls,
            report.summary.failed_tool_calls,
            report.summary.missing_expected_ref_rows
        )
        .into());
    }
    Ok(())
}

struct JsonProgressLogger {
    every: Option<usize>,
}

impl ReplayProgressObserver for JsonProgressLogger {
    fn observe(&self, progress: ReplayProgress<'_>) {
        let Some(every) = self.every else {
            return;
        };
        if every == 0 {
            return;
        }
        if !progress.processed.is_multiple_of(every) && progress.processed != progress.total {
            return;
        }
        eprintln!(
            "{}",
            json!({
                "event": "underpass_operator_mcp_replay.progress",
                "processed": progress.processed,
                "total": progress.total,
                "step_id": progress.row.step_id,
                "action": progress.row.action_label,
                "success": progress.row.success,
                "partial_result": progress.row.partial_result,
                "elapsed_ms": progress.elapsed_ms,
            })
        );
    }
}

struct KernelMcpToolCaller {
    server: KernelMcpServer,
}

impl ReplayToolCaller for KernelMcpToolCaller {
    fn call_tool<'a>(
        &'a self,
        request: ReplayToolCallRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ReplayToolCallResponse, ReplayApplicationError>> + Send + 'a,
        >,
    > {
        Box::pin(async move {
            call_mcp_tool(
                &self.server,
                request.request_id,
                &request.name,
                &request.arguments,
            )
            .await
            .map(|structured_content| ReplayToolCallResponse { structured_content })
            .map_err(|error| ReplayApplicationError::new(error.to_string()))
        })
    }
}

async fn call_mcp_tool(
    server: &KernelMcpServer,
    id: u64,
    name: &str,
    arguments: &Value,
) -> Result<Value, Box<dyn Error + Send + Sync>> {
    let request = json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": {
            "name": name,
            "arguments": arguments
        }
    });
    let response = server
        .handle_json_line(&request.to_string())
        .await
        .ok_or_else(|| format!("MCP tool `{name}` returned no JSON-RPC response"))?;
    let value = serde_json::from_str::<Value>(&response)?;
    if let Some(error) = value.get("error") {
        return Err(format!("MCP tool `{name}` returned JSON-RPC error: {error}").into());
    }
    let result = value
        .get("result")
        .ok_or_else(|| format!("MCP tool `{name}` returned no result"))?;
    if result
        .get("isError")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Err(format!("MCP tool `{name}` returned isError=true: {result}").into());
    }
    result
        .get("structuredContent")
        .cloned()
        .ok_or_else(|| format!("MCP tool `{name}` returned no structuredContent").into())
}

fn parse_args(
    mut args: impl Iterator<Item = String>,
) -> Result<Args, Box<dyn Error + Send + Sync>> {
    let mut trajectories = None;
    let mut predictions = None;
    let mut output = None;
    let mut endpoint = None;
    let mut limit = None;
    let mut offset = 0usize;
    let mut log_progress_every = None;
    let mut force = false;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--trajectories" => {
                trajectories = Some(PathBuf::from(next_arg(&mut args, "--trajectories")?));
            }
            "--predictions" => {
                predictions = Some(PathBuf::from(next_arg(&mut args, "--predictions")?));
            }
            "--output" => output = Some(PathBuf::from(next_arg(&mut args, "--output")?)),
            "--endpoint" => endpoint = Some(next_arg(&mut args, "--endpoint")?),
            "--limit" => limit = Some(next_arg(&mut args, "--limit")?.parse()?),
            "--offset" => offset = next_arg(&mut args, "--offset")?.parse()?,
            "--log-progress-every" => {
                log_progress_every = Some(next_arg(&mut args, "--log-progress-every")?.parse()?);
            }
            "--force" => force = true,
            "--help" | "-h" => return Err(usage().into()),
            value if value.starts_with('-') => {
                return Err(format!("unknown argument: {value}\n{}", usage()).into());
            }
            value => {
                if trajectories.is_some() {
                    return Err(format!("unexpected positional argument: {value}").into());
                }
                trajectories = Some(PathBuf::from(value));
            }
        }
    }

    Ok(Args {
        trajectories: trajectories.ok_or_else(usage)?,
        predictions: predictions.ok_or("--predictions is required")?,
        output: output.ok_or("--output is required")?,
        endpoint,
        limit,
        offset,
        log_progress_every,
        force,
    })
}

fn usage() -> String {
    "usage: underpass_operator_mcp_replay --trajectories <trajectories.jsonl> --predictions <predictions.jsonl> --output <dir> [--endpoint URL] [--limit n] [--offset n] [--log-progress-every n] [--force]".to_string()
}

fn next_arg(
    args: &mut impl Iterator<Item = String>,
    name: &str,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    args.next()
        .ok_or_else(|| format!("{name} requires a value").into())
}

fn select_trajectories(
    values: Vec<underpass_operator_replay_domain::ReplayTrajectory>,
    args: &Args,
) -> Vec<underpass_operator_replay_domain::ReplayTrajectory> {
    values
        .into_iter()
        .skip(args.offset)
        .take(args.limit.unwrap_or(usize::MAX))
        .collect()
}

fn now_unix_seconds() -> Result<u64, Box<dyn Error + Send + Sync>> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}
