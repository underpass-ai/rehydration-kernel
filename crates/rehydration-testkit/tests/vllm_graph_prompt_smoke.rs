use std::error::Error;

use rehydration_testkit::{
    LlmEvaluatorConfig, LlmProvider, llm_graph_to_projection_events, normalize_llm_json_response,
    parse_llm_graph_batch,
};
use serde::Deserialize;

const RUN_ENV: &str = "RUN_VLLM_SMOKE";
const ROOT_NODE_ID: &str = "incident-2026-04-08-payments-latency";
const REQUEST_FIXTURE: &str =
    include_str!("../../../api/examples/inference-prompts/vllm-graph-materialization.request.json");

#[derive(Debug, Deserialize)]
struct OpenAiChatResponse {
    choices: Vec<OpenAiChoice>,
    usage: Option<OpenAiUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: OpenAiMessage,
}

#[derive(Debug, Deserialize)]
struct OpenAiMessage {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
}

#[tokio::test]
async fn vllm_graph_prompt_smoke_returns_valid_batch() -> Result<(), Box<dyn Error + Send + Sync>> {
    if std::env::var(RUN_ENV).as_deref() != Ok("1") {
        eprintln!(
            "skipping vLLM graph smoke: set {RUN_ENV}=1 plus LLM_ENDPOINT/LLM_MODEL/LLM_PROVIDER"
        );
        return Ok(());
    }

    let config = LlmEvaluatorConfig::from_env();
    assert_eq!(
        config.provider,
        LlmProvider::OpenAI,
        "vLLM smoke expects LLM_PROVIDER=openai"
    );

    let client = build_http_client(&config)?;
    let request_body = build_request_body(&config)?;
    let (raw_content, prompt_tokens, completion_tokens) =
        send_openai_request(&client, &config, request_body).await?;

    let normalized = normalize_llm_json_response(&raw_content);
    let batch = parse_llm_graph_batch(&normalized)
        .map_err(|error| format!("failed to parse normalized response as LlmGraphBatch: {error}\nraw={raw_content}\nnormalized={normalized}"))?;

    assert_eq!(batch.root_node_id, ROOT_NODE_ID);
    assert_eq!(
        batch.nodes.len(),
        3,
        "model should keep the smoke graph compact"
    );
    assert_eq!(
        batch.relations.len(),
        2,
        "model should emit exactly the requested two root relations"
    );
    assert!(
        batch
            .relations
            .iter()
            .all(|relation| relation.source_node_id == ROOT_NODE_ID)
    );
    assert_eq!(
        batch.node_details.len(),
        2,
        "model should emit the requested two node details"
    );
    assert!(batch.nodes.iter().any(|node| {
        node.node_id != ROOT_NODE_ID
            && (node.title.to_ascii_lowercase().contains("db") || node.summary.contains("50 to 5"))
    }));
    assert!(batch.nodes.iter().any(|node| {
        node.node_id != ROOT_NODE_ID
            && (node.title.to_ascii_lowercase().contains("reroute")
                || node.summary.contains("80% of traffic"))
    }));
    assert!(
        batch
            .node_details
            .iter()
            .all(|detail| detail.node_id != ROOT_NODE_ID)
    );

    let messages =
        llm_graph_to_projection_events(&batch, "rehydration", "vllm-smoke").map_err(|error| {
            format!(
                "model returned a batch that did not translate: {error}\nnormalized={normalized}"
            )
        })?;
    assert_eq!(
        messages.len(),
        5,
        "3 node events + 2 node detail events expected for the smoke payload"
    );
    assert!(prompt_tokens > 0);
    assert!(completion_tokens > 0);

    Ok(())
}

fn build_request_body(
    config: &LlmEvaluatorConfig,
) -> Result<serde_json::Value, Box<dyn Error + Send + Sync>> {
    let mut body: serde_json::Value = serde_json::from_str(REQUEST_FIXTURE)?;

    body["model"] = serde_json::json!(config.model);
    body["temperature"] = serde_json::json!(config.temperature);
    body["max_tokens"] = serde_json::json!(config.max_tokens.max(2048));

    let disable_thinking = std::env::var("LLM_ENABLE_THINKING")
        .map(|v| v == "false" || v == "0")
        .unwrap_or(false);
    if disable_thinking {
        body["chat_template_kwargs"] = serde_json::json!({"enable_thinking": false});
    } else {
        body["thinking_token_budget"] = serde_json::json!(512);
    }

    Ok(body)
}

async fn send_openai_request(
    client: &reqwest::Client,
    config: &LlmEvaluatorConfig,
    body: serde_json::Value,
) -> Result<(String, u32, u32), Box<dyn Error + Send + Sync>> {
    let mut req = client.post(&config.endpoint).json(&body);
    if let Some(key) = config.api_key.as_deref() {
        req = req.bearer_auth(key);
    }

    let response = req.send().await?;
    let status = response.status();
    if !status.is_success() {
        let body_text = response.text().await.unwrap_or_default();
        return Err(format!("LLM request failed with {status}: {body_text}").into());
    }

    let chat: OpenAiChatResponse = response.json().await?;
    let content = chat
        .choices
        .first()
        .and_then(|choice| choice.message.content.clone())
        .unwrap_or_default();
    let usage = chat.usage.unwrap_or(OpenAiUsage {
        prompt_tokens: 0,
        completion_tokens: 0,
    });

    Ok((content, usage.prompt_tokens, usage.completion_tokens))
}

fn build_http_client(
    config: &LlmEvaluatorConfig,
) -> Result<reqwest::Client, Box<dyn Error + Send + Sync>> {
    let mut builder = reqwest::Client::builder();

    if config.tls_insecure {
        return Err(
            "LLM_TLS_INSECURE is no longer supported; require valid TLS certificates".into(),
        );
    }

    if let (Some(cert_path), Some(key_path)) = (&config.tls_cert_path, &config.tls_key_path) {
        let cert_pem = std::fs::read(cert_path)?;
        let key_pem = std::fs::read(key_path)?;
        let mut combined = cert_pem;
        combined.extend_from_slice(&key_pem);
        builder = builder.identity(reqwest::Identity::from_pem(&combined)?);
    }

    Ok(builder.build()?)
}
