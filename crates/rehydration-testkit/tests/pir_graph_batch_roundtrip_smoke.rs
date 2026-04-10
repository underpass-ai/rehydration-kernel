use std::env;
use std::error::Error;
use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use rehydration_testkit::{
    GraphBatch, GraphBatchSemanticClassifierPolicy, LlmEvaluatorConfig, call_llm,
    classify_graph_batch_semantic_classes_with_policy, namespace_graph_batch, parse_graph_batch,
};
use serde_json::Value;

const RUN_ENV: &str = "RUN_PIR_GRAPH_BATCH_SMOKE";
const CONTEXT_CONSUMPTION_RUN_ENV: &str = "RUN_PIR_GRAPH_BATCH_CONTEXT_CONSUMPTION_SMOKE";
const DEFAULT_BATCH_FIXTURE: &str =
    include_str!("../../../api/examples/kernel/v1beta1/async/incident-graph-batch.json");
const INCREMENTAL_BATCH_FIXTURE: &str = include_str!(
    "../../../api/examples/kernel/v1beta1/async/incident-graph-batch.incremental-2.json"
);
const KERNEL_CONTEXT_CONSUMPTION_PROMPT: &str =
    include_str!("../../../api/examples/inference-prompts/kernel-context-consumption.txt");

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[test]
fn pir_graph_batch_roundtrip_smoke_succeeds_against_live_kernel() -> TestResult<()> {
    if env::var(RUN_ENV).as_deref() != Ok("1") {
        eprintln!(
            "skipping PIR GraphBatch roundtrip smoke: set {RUN_ENV}=1 plus PIR_GRAPH_BATCH_NATS_URL and PIR_GRAPH_BATCH_GRPC_ENDPOINT"
        );
        return Ok(());
    }

    let run_id = env::var("PIR_GRAPH_BATCH_RUN_ID")
        .unwrap_or_else(|_| format!("pir-live-smoke-{}", unix_timestamp_secs()));
    let payload = load_fixture("PIR_GRAPH_BATCH_INPUT", DEFAULT_BATCH_FIXTURE)?;
    let mut batch = parse_graph_batch(&payload)?;
    namespace_graph_batch(&mut batch, &run_id);
    let summary = run_roundtrip(&serde_json::to_vec(&batch)?, &run_id, false, None)?;
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

#[tokio::test]
async fn pir_graph_batch_incremental_context_consumption_smoke_succeeds_against_live_kernel()
-> TestResult<()> {
    if env::var(CONTEXT_CONSUMPTION_RUN_ENV).as_deref() != Ok("1") {
        eprintln!(
            "skipping PIR GraphBatch context consumption smoke: set {CONTEXT_CONSUMPTION_RUN_ENV}=1 plus LLM_* and PIR_GRAPH_BATCH_* variables"
        );
        return Ok(());
    }

    let node_namespace = env::var("PIR_GRAPH_BATCH_NODE_NAMESPACE")
        .unwrap_or_else(|_| format!("pir-live-context-{}", unix_timestamp_secs()));
    let first_run_id = format!("{node_namespace}-wave-1");
    let second_run_id = format!("{node_namespace}-wave-2");

    let wave_one_payload = load_fixture("PIR_GRAPH_BATCH_INPUT", DEFAULT_BATCH_FIXTURE)?;
    let wave_two_payload = load_fixture(
        "PIR_GRAPH_BATCH_INCREMENTAL_INPUT",
        INCREMENTAL_BATCH_FIXTURE,
    )?;
    let config = LlmEvaluatorConfig::from_env();

    let mut batch_one = parse_graph_batch(&wave_one_payload)?;
    namespace_graph_batch(&mut batch_one, &node_namespace);
    let batch_one = classify_batch_with_semantic_reranker(&config, &batch_one, &first_run_id)
        .await?
        .batch;
    let summary_one = run_roundtrip(&serde_json::to_vec(&batch_one)?, &first_run_id, false, None)?;

    assert_eq!(
        summary_one.get("root_node_id").and_then(Value::as_str),
        Some(batch_one.root_node_id.as_str())
    );
    assert_eq!(
        summary_one.get("run_id").and_then(Value::as_str),
        Some(first_run_id.as_str())
    );
    assert_eq!(
        summary_one.get("neighbor_count").and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        summary_one
            .get("relationship_count")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        summary_one.get("detail_count").and_then(Value::as_u64),
        Some(2)
    );

    let mut batch_two = parse_graph_batch(&wave_two_payload)?;
    namespace_graph_batch(&mut batch_two, &node_namespace);
    let batch_two = classify_batch_with_semantic_reranker(&config, &batch_two, &second_run_id)
        .await?
        .batch;
    let summary_two = run_roundtrip(
        &serde_json::to_vec(&batch_two)?,
        &second_run_id,
        true,
        Some("reason_preserving"),
    )?;

    assert_eq!(
        summary_two.get("root_node_id").and_then(Value::as_str),
        Some(batch_two.root_node_id.as_str())
    );
    assert_eq!(
        summary_two.get("root_node_id").and_then(Value::as_str),
        summary_one.get("root_node_id").and_then(Value::as_str)
    );
    assert_eq!(
        summary_two.get("run_id").and_then(Value::as_str),
        Some(second_run_id.as_str())
    );
    assert_eq!(
        summary_two.get("neighbor_count").and_then(Value::as_u64),
        Some(4)
    );
    assert_eq!(
        summary_two
            .get("relationship_count")
            .and_then(Value::as_u64),
        Some(4)
    );
    assert_eq!(
        summary_two.get("detail_count").and_then(Value::as_u64),
        Some(4)
    );

    let rendered_context = summary_two
        .get("rendered_content")
        .and_then(Value::as_str)
        .ok_or("roundtrip summary should include rendered_content")?;
    assert!(
        rendered_context.contains("Retry storm amplified load"),
        "rendered content should include the second-wave finding"
    );
    assert!(
        rendered_context.contains("Apply retry cap"),
        "rendered content should include the second-wave task"
    );

    let answer = answer_from_rehydrated_context(
        &config,
        rendered_context,
        "Answer in one concise sentence: what new finding amplified the incident after the pool regression, and what task was planned to mitigate it?",
    )
    .await?;

    assert_answer_mentions_any(
        &answer,
        &["retry storm", "retries", "retry"],
        "second-wave finding",
    )?;
    assert_answer_mentions_any(
        &answer,
        &["retry cap", "cap retries", "full jitter"],
        "second-wave task",
    )?;

    eprintln!(
        "pir context consumption smoke node_namespace={node_namespace} rendered_chars={} answer={}",
        rendered_context.chars().count(),
        answer.replace('\n', " ")
    );

    Ok(())
}

async fn classify_batch_with_semantic_reranker(
    config: &LlmEvaluatorConfig,
    batch: &GraphBatch,
    run_id: &str,
) -> TestResult<rehydration_testkit::GraphBatchSemanticClassifierOutcome> {
    let outcome = classify_graph_batch_semantic_classes_with_policy(
        config,
        batch,
        "rehydration",
        run_id,
        GraphBatchSemanticClassifierPolicy::from_env(),
    )
    .await?;
    assert!(
        outcome.attempts >= 1,
        "semantic reranker must run for run_id={run_id}"
    );
    Ok(outcome)
}

fn load_fixture(env_key: &str, default_fixture: &str) -> TestResult<String> {
    match env::var(env_key) {
        Ok(path) if !path.trim().is_empty() => Ok(fs::read_to_string(path)?),
        _ => Ok(default_fixture.to_string()),
    }
}

fn run_roundtrip(
    batch_payload: &[u8],
    run_id: &str,
    include_rendered_content: bool,
    rehydration_mode: Option<&str>,
) -> TestResult<Value> {
    let nats_url = required_env("PIR_GRAPH_BATCH_NATS_URL")?;
    let grpc_endpoint = required_env("PIR_GRAPH_BATCH_GRPC_ENDPOINT")?;
    let role =
        env::var("PIR_GRAPH_BATCH_ROLE").unwrap_or_else(|_| "incident-commander".to_string());
    let scopes = env::var("PIR_GRAPH_BATCH_SCOPES").unwrap_or_else(|_| "graph,details".to_string());

    let mut command = Command::new(graph_batch_roundtrip_bin());
    command
        .arg("--input")
        .arg("-")
        .arg("--nats-url")
        .arg(nats_url)
        .arg("--grpc-endpoint")
        .arg(grpc_endpoint)
        .arg("--run-id")
        .arg(run_id)
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
    if include_rendered_content {
        command.arg("--include-rendered-content");
    }
    if let Some(rehydration_mode) = rehydration_mode {
        command.arg("--rehydration-mode").arg(rehydration_mode);
    }

    let mut child = command.spawn()?;
    child
        .stdin
        .as_mut()
        .ok_or("roundtrip stdin should be available")?
        .write_all(batch_payload)?;
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

    Ok(serde_json::from_slice(&output.stdout)?)
}

async fn answer_from_rehydrated_context(
    config: &LlmEvaluatorConfig,
    rendered_context: &str,
    question: &str,
) -> TestResult<String> {
    let client = reqwest::Client::builder().build()?;
    let prompt = KERNEL_CONTEXT_CONSUMPTION_PROMPT
        .replace("{question}", question)
        .replace("{rendered_context}", rendered_context);
    let (answer, _prompt_tokens, _completion_tokens) = call_llm(
        &client,
        &config.endpoint,
        &config.model,
        config.provider,
        config.api_key.as_deref(),
        &prompt,
        config.max_tokens.clamp(128, 512),
        0.0,
    )
    .await?;
    Ok(answer)
}

fn assert_answer_mentions_any(answer: &str, needles: &[&str], label: &str) -> TestResult<()> {
    let normalized = answer.to_ascii_lowercase();
    if needles
        .iter()
        .any(|needle| normalized.contains(&needle.to_ascii_lowercase()))
    {
        return Ok(());
    }

    Err(format!("LLM answer did not mention {label}; answer={answer}").into())
}

fn required_env(key: &str) -> TestResult<String> {
    env::var(key).map_err(|_| format!("missing env `{key}`").into())
}

fn graph_batch_roundtrip_bin() -> String {
    env::var("GRAPH_BATCH_ROUNDTRIP_BIN")
        .unwrap_or_else(|_| env!("CARGO_BIN_EXE_graph_batch_roundtrip").to_string())
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
