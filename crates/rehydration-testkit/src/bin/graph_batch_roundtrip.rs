use std::env;
use std::error::Error;
use std::fs;
use std::io;
use std::io::Read;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use async_nats::{Client, ConnectOptions};
use rehydration_proto::v1beta1::{
    GetContextRequest, GetContextResponse, GetNodeDetailRequest, GetNodeDetailResponse,
    context_query_service_client::ContextQueryServiceClient,
};
use rehydration_testkit::{GraphBatch, graph_batch_to_projection_events, parse_graph_batch};
use reqwest::Url;
use serde::Serialize;
use tokio::time::sleep;
use tonic::Code;
use tonic::transport::{Certificate, Channel, ClientTlsConfig, Endpoint, Identity};

#[derive(Debug, Clone, PartialEq, Eq)]
struct Args {
    input: String,
    nats_url: String,
    grpc_endpoint: String,
    subject_prefix: String,
    run_id: String,
    role: String,
    requested_scopes: Vec<String>,
    depth: u32,
    token_budget: u32,
    detail_node_id: Option<String>,
    wait_timeout_secs: u64,
    poll_interval_ms: u64,
    grpc_tls_ca_path: Option<String>,
    grpc_tls_cert_path: Option<String>,
    grpc_tls_key_path: Option<String>,
    grpc_tls_domain_name: Option<String>,
    nats_tls_ca_path: Option<String>,
    nats_tls_cert_path: Option<String>,
    nats_tls_key_path: Option<String>,
    nats_tls_first: bool,
}

#[derive(Debug, Serialize)]
struct RoundtripSummary {
    root_node_id: String,
    run_id: String,
    published_messages: usize,
    subject_prefix: String,
    grpc_endpoint: String,
    nats_url: String,
    role: String,
    requested_scopes: Vec<String>,
    depth: u32,
    token_budget: u32,
    selected_detail_node_id: Option<String>,
    bundle_role_count: usize,
    neighbor_count: usize,
    relationship_count: usize,
    detail_count: usize,
    rendered_chars: usize,
    detail_revision: Option<u64>,
    detail_excerpt: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    install_crypto_provider();
    let args = parse_args(env::args().skip(1))?;
    let payload = read_input_payload(&args.input)?;
    let batch = parse_graph_batch(&payload)?;
    let selected_detail_node_id = select_detail_node_id(&args, &batch);
    let messages = graph_batch_to_projection_events(&batch, &args.subject_prefix, &args.run_id)?;

    let nats_client = connect_nats(&args).await?;
    publish_messages(&nats_client, &messages).await?;

    let mut query_client = connect_query_client(&args).await?;
    let (context, detail) = wait_for_projection(
        &mut query_client,
        &batch.root_node_id,
        &args,
        selected_detail_node_id.as_deref(),
    )
    .await?;

    let summary = build_summary(
        &batch,
        &args,
        messages.len(),
        selected_detail_node_id,
        context,
        detail,
    );

    println!("{}", serde_json::to_string_pretty(&summary)?);

    Ok(())
}

fn read_input_payload(input: &str) -> Result<String, Box<dyn Error + Send + Sync>> {
    if input == "-" {
        let mut payload = String::new();
        io::stdin().read_to_string(&mut payload)?;
        return Ok(payload);
    }

    Ok(fs::read_to_string(input)?)
}

fn build_summary(
    batch: &GraphBatch,
    args: &Args,
    published_messages: usize,
    selected_detail_node_id: Option<String>,
    context: GetContextResponse,
    detail: Option<GetNodeDetailResponse>,
) -> RoundtripSummary {
    let bundle = context.bundle.unwrap_or_default();
    let role_bundle = bundle.bundles.first();
    let rendered = context.rendered.unwrap_or_default();

    let detail_revision = detail
        .as_ref()
        .and_then(|response| response.detail.as_ref())
        .map(|detail| detail.revision);
    let detail_excerpt = detail
        .as_ref()
        .and_then(|response| response.detail.as_ref())
        .map(|detail| excerpt(&detail.detail, 180));

    RoundtripSummary {
        root_node_id: batch.root_node_id.clone(),
        run_id: args.run_id.clone(),
        published_messages,
        subject_prefix: args.subject_prefix.clone(),
        grpc_endpoint: args.grpc_endpoint.clone(),
        nats_url: args.nats_url.clone(),
        role: args.role.clone(),
        requested_scopes: args.requested_scopes.clone(),
        depth: args.depth,
        token_budget: args.token_budget,
        selected_detail_node_id,
        bundle_role_count: bundle.bundles.len(),
        neighbor_count: role_bundle
            .map(|bundle| bundle.neighbor_nodes.len())
            .unwrap_or(0),
        relationship_count: role_bundle
            .map(|bundle| bundle.relationships.len())
            .unwrap_or(0),
        detail_count: role_bundle
            .map(|bundle| bundle.node_details.len())
            .unwrap_or(0),
        rendered_chars: rendered.content.chars().count(),
        detail_revision,
        detail_excerpt,
    }
}

async fn publish_messages(
    client: &Client,
    messages: &[(String, Vec<u8>)],
) -> Result<(), Box<dyn Error + Send + Sync>> {
    for (subject, payload) in messages {
        client
            .publish(subject.clone(), payload.clone().into())
            .await?;
    }
    client.flush().await?;
    Ok(())
}

async fn wait_for_projection(
    query_client: &mut ContextQueryServiceClient<Channel>,
    root_node_id: &str,
    args: &Args,
    detail_node_id: Option<&str>,
) -> Result<(GetContextResponse, Option<GetNodeDetailResponse>), Box<dyn Error + Send + Sync>> {
    let deadline = Instant::now() + Duration::from_secs(args.wait_timeout_secs);
    let poll_interval = Duration::from_millis(args.poll_interval_ms);

    loop {
        match query_client
            .get_context(GetContextRequest {
                root_node_id: root_node_id.to_string(),
                role: args.role.clone(),
                token_budget: args.token_budget,
                requested_scopes: args.requested_scopes.clone(),
                depth: args.depth,
                max_tier: 0,
                rehydration_mode: 0,
            })
            .await
        {
            Ok(response) => {
                let context = response.into_inner();
                if let Some(node_id) = detail_node_id {
                    match query_client
                        .get_node_detail(GetNodeDetailRequest {
                            node_id: node_id.to_string(),
                        })
                        .await
                    {
                        Ok(detail) => return Ok((context, Some(detail.into_inner()))),
                        Err(status) if should_retry_grpc_status(&status) => {}
                        Err(status) => return Err(Box::new(status)),
                    }
                } else {
                    return Ok((context, None));
                }
            }
            Err(status) if should_retry_grpc_status(&status) => {}
            Err(status) => return Err(Box::new(status)),
        }

        if Instant::now() >= deadline {
            return Err(Box::new(io::Error::new(
                io::ErrorKind::TimedOut,
                format!(
                    "projection did not become queryable for root `{root_node_id}` within {}s",
                    args.wait_timeout_secs
                ),
            )));
        }

        sleep(poll_interval).await;
    }
}

fn should_retry_grpc_status(status: &tonic::Status) -> bool {
    matches!(
        status.code(),
        Code::NotFound | Code::Unavailable | Code::DeadlineExceeded | Code::Unknown
    )
}

async fn connect_query_client(
    args: &Args,
) -> Result<ContextQueryServiceClient<Channel>, Box<dyn Error + Send + Sync>> {
    let endpoint = build_grpc_endpoint(args)?;
    let channel = endpoint.connect().await?;
    Ok(ContextQueryServiceClient::new(channel))
}

fn build_grpc_endpoint(args: &Args) -> Result<Endpoint, Box<dyn Error + Send + Sync>> {
    let mut endpoint = Endpoint::from_shared(args.grpc_endpoint.clone())?;

    if needs_grpc_tls(args) {
        let domain_name = args
            .grpc_tls_domain_name
            .clone()
            .or_else(|| host_from_url(&args.grpc_endpoint))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "gRPC TLS requires a domain name or a host in the endpoint URL",
                )
            })?;

        let mut tls = ClientTlsConfig::new().domain_name(domain_name);

        if let Some(ca_path) = &args.grpc_tls_ca_path {
            tls = tls.ca_certificate(Certificate::from_pem(fs::read(ca_path)?));
        }

        match (&args.grpc_tls_cert_path, &args.grpc_tls_key_path) {
            (Some(cert_path), Some(key_path)) => {
                tls = tls.identity(Identity::from_pem(
                    fs::read(cert_path)?,
                    fs::read(key_path)?,
                ));
            }
            (None, None) => {}
            _ => {
                return Err(Box::new(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "gRPC client identity requires both --grpc-tls-cert-path and --grpc-tls-key-path",
                )));
            }
        }

        endpoint = endpoint.tls_config(tls)?;
    }

    Ok(endpoint)
}

async fn connect_nats(args: &Args) -> Result<Client, Box<dyn Error + Send + Sync>> {
    let mut options = ConnectOptions::new();

    if needs_nats_tls(args) {
        options = options.require_tls(true);

        if let Some(ca_path) = &args.nats_tls_ca_path {
            options = options.add_root_certificates(PathBuf::from(ca_path));
        }

        match (&args.nats_tls_cert_path, &args.nats_tls_key_path) {
            (Some(cert_path), Some(key_path)) => {
                options = options
                    .add_client_certificate(PathBuf::from(cert_path), PathBuf::from(key_path));
            }
            (None, None) => {}
            _ => {
                return Err(Box::new(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "NATS client identity requires both --nats-tls-cert-path and --nats-tls-key-path",
                )));
            }
        }

        if args.nats_tls_first {
            options = options.tls_first();
        }
    } else if args.nats_tls_first {
        return Err(Box::new(io::Error::new(
            io::ErrorKind::InvalidInput,
            "--nats-tls-first requires TLS",
        )));
    }

    Ok(options.connect(&args.nats_url).await?)
}

fn needs_grpc_tls(args: &Args) -> bool {
    args.grpc_endpoint.starts_with("https://")
        || args.grpc_tls_ca_path.is_some()
        || args.grpc_tls_cert_path.is_some()
        || args.grpc_tls_key_path.is_some()
        || args.grpc_tls_domain_name.is_some()
}

fn needs_nats_tls(args: &Args) -> bool {
    args.nats_url.starts_with("tls://")
        || args.nats_tls_ca_path.is_some()
        || args.nats_tls_cert_path.is_some()
        || args.nats_tls_key_path.is_some()
}

fn select_detail_node_id(args: &Args, batch: &GraphBatch) -> Option<String> {
    args.detail_node_id.clone().or_else(|| {
        batch
            .node_details
            .first()
            .map(|detail| detail.node_id.clone())
    })
}

fn host_from_url(value: &str) -> Option<String> {
    Url::parse(value)
        .ok()
        .and_then(|url| url.host_str().map(ToString::to_string))
}

fn excerpt(value: &str, max_chars: usize) -> String {
    let total = value.chars().count();
    if total <= max_chars {
        return value.to_string();
    }
    let head: String = value.chars().take(max_chars).collect();
    format!("{head}...")
}

fn install_crypto_provider() {
    let _ = tokio_rustls::rustls::crypto::aws_lc_rs::default_provider().install_default();
}

fn parse_args(
    mut args: impl Iterator<Item = String>,
) -> Result<Args, Box<dyn Error + Send + Sync>> {
    let mut input = None;
    let mut nats_url = None;
    let mut grpc_endpoint = None;
    let mut subject_prefix = "rehydration".to_string();
    let mut run_id = None;
    let mut role = "developer".to_string();
    let mut requested_scopes = vec!["graph".to_string(), "details".to_string()];
    let mut depth = 2_u32;
    let mut token_budget = 2048_u32;
    let mut detail_node_id = None;
    let mut wait_timeout_secs = 20_u64;
    let mut poll_interval_ms = 250_u64;
    let mut grpc_tls_ca_path = None;
    let mut grpc_tls_cert_path = None;
    let mut grpc_tls_key_path = None;
    let mut grpc_tls_domain_name = None;
    let mut nats_tls_ca_path = None;
    let mut nats_tls_cert_path = None;
    let mut nats_tls_key_path = None;
    let mut nats_tls_first = false;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--input" => input = args.next(),
            "--nats-url" => nats_url = args.next(),
            "--grpc-endpoint" => grpc_endpoint = args.next(),
            "--subject-prefix" => {
                subject_prefix = required_flag_value(&mut args, "--subject-prefix")?
            }
            "--run-id" => run_id = Some(required_flag_value(&mut args, "--run-id")?),
            "--role" => role = required_flag_value(&mut args, "--role")?,
            "--requested-scope" => {
                if requested_scopes == ["graph".to_string(), "details".to_string()] {
                    requested_scopes.clear();
                }
                requested_scopes.push(required_flag_value(&mut args, "--requested-scope")?);
            }
            "--depth" => depth = parse_u32_flag(&mut args, "--depth")?,
            "--token-budget" => token_budget = parse_u32_flag(&mut args, "--token-budget")?,
            "--detail-node-id" => {
                detail_node_id = Some(required_flag_value(&mut args, "--detail-node-id")?)
            }
            "--wait-timeout-secs" => {
                wait_timeout_secs = parse_u64_flag(&mut args, "--wait-timeout-secs")?
            }
            "--poll-interval-ms" => {
                poll_interval_ms = parse_u64_flag(&mut args, "--poll-interval-ms")?
            }
            "--grpc-tls-ca-path" => {
                grpc_tls_ca_path = Some(required_flag_value(&mut args, "--grpc-tls-ca-path")?)
            }
            "--grpc-tls-cert-path" => {
                grpc_tls_cert_path = Some(required_flag_value(&mut args, "--grpc-tls-cert-path")?)
            }
            "--grpc-tls-key-path" => {
                grpc_tls_key_path = Some(required_flag_value(&mut args, "--grpc-tls-key-path")?)
            }
            "--grpc-tls-domain-name" => {
                grpc_tls_domain_name =
                    Some(required_flag_value(&mut args, "--grpc-tls-domain-name")?)
            }
            "--nats-tls-ca-path" => {
                nats_tls_ca_path = Some(required_flag_value(&mut args, "--nats-tls-ca-path")?)
            }
            "--nats-tls-cert-path" => {
                nats_tls_cert_path = Some(required_flag_value(&mut args, "--nats-tls-cert-path")?)
            }
            "--nats-tls-key-path" => {
                nats_tls_key_path = Some(required_flag_value(&mut args, "--nats-tls-key-path")?)
            }
            "--nats-tls-first" => nats_tls_first = true,
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            other => {
                return Err(Box::new(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unknown argument `{other}`"),
                )));
            }
        }
    }

    Ok(Args {
        input: required_value(input, "--input")?,
        nats_url: required_value(nats_url, "--nats-url")?,
        grpc_endpoint: required_value(grpc_endpoint, "--grpc-endpoint")?,
        subject_prefix,
        run_id: required_value(run_id, "--run-id")?,
        role,
        requested_scopes,
        depth,
        token_budget,
        detail_node_id,
        wait_timeout_secs,
        poll_interval_ms,
        grpc_tls_ca_path,
        grpc_tls_cert_path,
        grpc_tls_key_path,
        grpc_tls_domain_name,
        nats_tls_ca_path,
        nats_tls_cert_path,
        nats_tls_key_path,
        nats_tls_first,
    })
}

fn required_value(
    value: Option<String>,
    flag: &str,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    value.ok_or_else(|| {
        Box::new(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{flag} is required"),
        )) as Box<dyn Error + Send + Sync>
    })
}

fn required_flag_value(
    args: &mut impl Iterator<Item = String>,
    flag: &str,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    required_value(args.next(), flag)
}

fn parse_u32_flag(
    args: &mut impl Iterator<Item = String>,
    flag: &str,
) -> Result<u32, Box<dyn Error + Send + Sync>> {
    let value = required_flag_value(args, flag)?;
    value.parse::<u32>().map_err(|error| {
        Box::new(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid value for {flag}: {error}"),
        )) as Box<dyn Error + Send + Sync>
    })
}

fn parse_u64_flag(
    args: &mut impl Iterator<Item = String>,
    flag: &str,
) -> Result<u64, Box<dyn Error + Send + Sync>> {
    let value = required_flag_value(args, flag)?;
    value.parse::<u64>().map_err(|error| {
        Box::new(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid value for {flag}: {error}"),
        )) as Box<dyn Error + Send + Sync>
    })
}

fn print_usage() {
    eprintln!(
        "usage: graph_batch_roundtrip --input <path|-> --nats-url <url> --grpc-endpoint <url> --run-id <id> [--subject-prefix <prefix>] [--role <role>] [--requested-scope <scope>] [--depth <n>] [--token-budget <n>] [--detail-node-id <node>] [--wait-timeout-secs <n>] [--poll-interval-ms <n>] [--grpc-tls-ca-path <path>] [--grpc-tls-cert-path <path>] [--grpc-tls-key-path <path>] [--grpc-tls-domain-name <name>] [--nats-tls-ca-path <path>] [--nats-tls-cert-path <path>] [--nats-tls-key-path <path>] [--nats-tls-first]"
    );
}

#[cfg(test)]
mod tests {
    use super::{Args, host_from_url, parse_args, select_detail_node_id};
    use rehydration_testkit::parse_graph_batch;

    const INCIDENT_BATCH: &str =
        include_str!("../../../../api/examples/kernel/v1beta1/async/incident-graph-batch.json");

    #[test]
    fn parse_args_reads_required_and_optional_values() {
        let args = parse_args(
            [
                "--input",
                "batch.json",
                "--nats-url",
                "tls://nats.example:4222",
                "--grpc-endpoint",
                "https://kernel.example.com",
                "--run-id",
                "pir-wave-1",
                "--requested-scope",
                "graph",
                "--requested-scope",
                "details",
                "--role",
                "incident-commander",
                "--depth",
                "3",
                "--token-budget",
                "4096",
                "--grpc-tls-domain-name",
                "kernel.example.com",
                "--nats-tls-first",
            ]
            .into_iter()
            .map(ToString::to_string),
        )
        .expect("args should parse");

        assert_eq!(
            args,
            Args {
                input: "batch.json".to_string(),
                nats_url: "tls://nats.example:4222".to_string(),
                grpc_endpoint: "https://kernel.example.com".to_string(),
                subject_prefix: "rehydration".to_string(),
                run_id: "pir-wave-1".to_string(),
                role: "incident-commander".to_string(),
                requested_scopes: vec!["graph".to_string(), "details".to_string()],
                depth: 3,
                token_budget: 4096,
                detail_node_id: None,
                wait_timeout_secs: 20,
                poll_interval_ms: 250,
                grpc_tls_ca_path: None,
                grpc_tls_cert_path: None,
                grpc_tls_key_path: None,
                grpc_tls_domain_name: Some("kernel.example.com".to_string()),
                nats_tls_ca_path: None,
                nats_tls_cert_path: None,
                nats_tls_key_path: None,
                nats_tls_first: true,
            }
        );
    }

    #[test]
    fn select_detail_node_defaults_to_first_batch_detail() {
        let batch = parse_graph_batch(INCIDENT_BATCH).expect("incident batch should parse");
        let args = parse_args(
            [
                "--input",
                "batch.json",
                "--nats-url",
                "nats://localhost:4222",
                "--grpc-endpoint",
                "http://localhost:50054",
                "--run-id",
                "wave-1",
            ]
            .into_iter()
            .map(ToString::to_string),
        )
        .expect("args should parse");

        assert_eq!(
            select_detail_node_id(&args, &batch).as_deref(),
            Some("finding:pir-2026-04-09-payments-latency:db-pool-typo")
        );
    }

    #[test]
    fn host_from_url_extracts_hostname() {
        assert_eq!(
            host_from_url("https://rehydration-kernel.underpassai.com:443"),
            Some("rehydration-kernel.underpassai.com".to_string())
        );
    }
}
