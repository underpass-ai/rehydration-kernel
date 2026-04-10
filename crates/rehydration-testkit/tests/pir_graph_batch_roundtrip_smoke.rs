use std::env;
use std::error::Error;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use rehydration_testkit::{namespace_graph_batch, parse_graph_batch};
use serde_json::Value;

const RUN_ENV: &str = "RUN_PIR_GRAPH_BATCH_SMOKE";
const DEFAULT_INPUT: &str = "api/examples/kernel/v1beta1/async/incident-graph-batch.json";

#[test]
fn pir_graph_batch_roundtrip_smoke_succeeds_against_live_kernel() -> Result<(), Box<dyn Error>> {
    if env::var(RUN_ENV).as_deref() != Ok("1") {
        eprintln!(
            "skipping PIR GraphBatch roundtrip smoke: set {RUN_ENV}=1 plus PIR_GRAPH_BATCH_NATS_URL and PIR_GRAPH_BATCH_GRPC_ENDPOINT"
        );
        return Ok(());
    }

    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .ok_or("workspace root should resolve")?
        .to_path_buf();

    let input = env::var("PIR_GRAPH_BATCH_INPUT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| repo_root.join(DEFAULT_INPUT));
    let nats_url = required_env("PIR_GRAPH_BATCH_NATS_URL")?;
    let grpc_endpoint = required_env("PIR_GRAPH_BATCH_GRPC_ENDPOINT")?;
    let role =
        env::var("PIR_GRAPH_BATCH_ROLE").unwrap_or_else(|_| "incident-commander".to_string());
    let scopes = env::var("PIR_GRAPH_BATCH_SCOPES").unwrap_or_else(|_| "graph,details".to_string());
    let run_id = env::var("PIR_GRAPH_BATCH_RUN_ID")
        .unwrap_or_else(|_| format!("pir-live-smoke-{}", unix_timestamp_secs()));
    let payload = fs::read_to_string(&input)?;
    let mut batch = parse_graph_batch(&payload)?;
    namespace_graph_batch(&mut batch, &run_id);
    let batch_payload = serde_json::to_vec(&batch)?;

    let mut command = Command::new(env!("CARGO_BIN_EXE_graph_batch_roundtrip"));
    command
        .arg("--input")
        .arg("-")
        .arg("--nats-url")
        .arg(nats_url)
        .arg("--grpc-endpoint")
        .arg(grpc_endpoint)
        .arg("--run-id")
        .arg(&run_id)
        .arg("--role")
        .arg(role)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    for scope in scopes
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        command.arg("--requested-scope").arg(scope);
    }

    add_optional_flag(
        &mut command,
        "PIR_GRAPH_BATCH_DETAIL_NODE_ID",
        "--detail-node-id",
    );
    add_optional_flag(
        &mut command,
        "PIR_GRAPH_BATCH_GRPC_TLS_CA_PATH",
        "--grpc-tls-ca-path",
    );
    add_optional_flag(
        &mut command,
        "PIR_GRAPH_BATCH_GRPC_TLS_CERT_PATH",
        "--grpc-tls-cert-path",
    );
    add_optional_flag(
        &mut command,
        "PIR_GRAPH_BATCH_GRPC_TLS_KEY_PATH",
        "--grpc-tls-key-path",
    );
    add_optional_flag(
        &mut command,
        "PIR_GRAPH_BATCH_GRPC_TLS_DOMAIN_NAME",
        "--grpc-tls-domain-name",
    );
    add_optional_flag(
        &mut command,
        "PIR_GRAPH_BATCH_NATS_TLS_CA_PATH",
        "--nats-tls-ca-path",
    );
    add_optional_flag(
        &mut command,
        "PIR_GRAPH_BATCH_NATS_TLS_CERT_PATH",
        "--nats-tls-cert-path",
    );
    add_optional_flag(
        &mut command,
        "PIR_GRAPH_BATCH_NATS_TLS_KEY_PATH",
        "--nats-tls-key-path",
    );

    if env::var("PIR_GRAPH_BATCH_NATS_TLS_FIRST").as_deref() == Ok("true") {
        command.arg("--nats-tls-first");
    }

    let mut child = command.spawn()?;
    child
        .stdin
        .as_mut()
        .ok_or("roundtrip stdin should be available")?
        .write_all(&batch_payload)?;
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(format!(
            "graph_batch_roundtrip failed with status {:?}\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    let summary: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        summary.get("root_node_id").and_then(Value::as_str),
        Some(batch.root_node_id.as_str())
    );
    assert_eq!(
        summary.get("run_id").and_then(Value::as_str),
        Some(run_id.as_str())
    );
    assert!(
        summary
            .get("published_messages")
            .and_then(Value::as_u64)
            .unwrap_or_default()
            >= 4
    );
    assert!(
        summary
            .get("neighbor_count")
            .and_then(Value::as_u64)
            .unwrap_or_default()
            >= 2
    );
    assert!(
        summary
            .get("detail_count")
            .and_then(Value::as_u64)
            .unwrap_or_default()
            >= 2
    );
    assert!(
        summary
            .get("rendered_chars")
            .and_then(Value::as_u64)
            .unwrap_or_default()
            > 0
    );

    Ok(())
}

fn required_env(key: &str) -> Result<String, Box<dyn Error>> {
    env::var(key).map_err(|_| format!("missing env `{key}`").into())
}

fn add_optional_flag(command: &mut Command, env_key: &str, flag: &str) {
    if let Ok(value) = env::var(env_key)
        && !value.trim().is_empty()
    {
        command.arg(flag).arg(value);
    }
}

fn unix_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
