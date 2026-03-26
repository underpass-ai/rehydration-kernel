//! LLM-in-the-loop evaluator for paper use cases.
//!
//! Sends rehydrated rendered context to an LLM endpoint (OpenAI-compatible,
//! OpenAI new-style, or Anthropic Claude) and evaluates the response against
//! ground truth.
//!
//! Prompts are loaded from `resources/llm_prompts.yaml` (compiled in via
//! `include_str!`) and can be overridden at runtime via `LLM_PROMPTS_PATH`.

use std::error::Error;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

// ── Prompt config ────────────────────────────────────────────────────

/// Prompt templates loaded from YAML.
#[derive(Debug, Clone, Deserialize)]
pub struct PromptConfig {
    pub inference_prompt: String,
    pub judge_prompt: String,
    pub judge_max_tokens: u32,
}

impl PromptConfig {
    /// Load from a YAML file path. Returns the compiled-in default if path is None.
    pub fn load(path: Option<&str>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let yaml = match path {
            Some(p) => std::fs::read_to_string(p)?,
            None => include_str!("../resources/llm_prompts.yaml").to_string(),
        };
        Ok(serde_yaml::from_str(&yaml)?)
    }
}

static PROMPT_CONFIG: LazyLock<PromptConfig> = LazyLock::new(|| {
    let yaml = match std::env::var("LLM_PROMPTS_PATH") {
        Ok(path) => std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read prompts from {path}: {e}")),
        Err(_) => include_str!("../resources/llm_prompts.yaml").to_string(),
    };
    serde_yaml::from_str(&yaml).expect("failed to parse llm_prompts.yaml")
});

/// Supported LLM API providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LlmProvider {
    /// OpenAI-compatible (vLLM, OpenAI GPT-4.1, etc.)
    OpenAI,
    /// OpenAI models that require max_completion_tokens (GPT-5.x, o3, o4)
    OpenAINew,
    /// Anthropic Claude API
    Anthropic,
}

/// Configuration for the LLM evaluator.
#[derive(Debug, Clone)]
pub struct LlmEvaluatorConfig {
    /// API endpoint.
    pub endpoint: String,
    /// Model name to use.
    pub model: String,
    /// API provider.
    pub provider: LlmProvider,
    /// API key (for OpenAI/Anthropic).
    pub api_key: Option<String>,
    /// Max tokens for the response.
    pub max_tokens: u32,
    /// Temperature (0.0 = deterministic).
    pub temperature: f64,
    /// Optional TLS client cert path for mTLS.
    pub tls_cert_path: Option<String>,
    /// Optional TLS client key path for mTLS.
    pub tls_key_path: Option<String>,
    /// Whether to skip TLS verification.
    pub tls_insecure: bool,
    /// Separate endpoint for the LLM-as-judge. Falls back to main endpoint.
    pub judge_endpoint: Option<String>,
    /// Separate model for the judge. Falls back to main model.
    pub judge_model: Option<String>,
    /// Provider for the judge. Falls back to main provider.
    pub judge_provider: Option<LlmProvider>,
    /// API key for judge. Falls back to main api_key.
    pub judge_api_key: Option<String>,
}

impl LlmEvaluatorConfig {
    pub fn from_env() -> Self {
        let model = std::env::var("LLM_MODEL").unwrap_or_else(|_| "Qwen/Qwen3-8B".to_string());
        let provider = detect_provider(&std::env::var("LLM_PROVIDER").unwrap_or_default(), &model);
        let judge_model = std::env::var("LLM_JUDGE_MODEL").ok();
        let judge_provider = judge_model
            .as_deref()
            .map(|m| detect_provider(&std::env::var("LLM_JUDGE_PROVIDER").unwrap_or_default(), m));
        Self {
            endpoint: std::env::var("LLM_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:8000/v1/chat/completions".to_string()),
            model,
            provider,
            api_key: std::env::var("LLM_API_KEY").ok(),
            max_tokens: std::env::var("LLM_MAX_TOKENS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(200),
            temperature: std::env::var("LLM_TEMPERATURE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.1),
            tls_cert_path: std::env::var("LLM_TLS_CERT_PATH").ok(),
            tls_key_path: std::env::var("LLM_TLS_KEY_PATH").ok(),
            tls_insecure: std::env::var("LLM_TLS_INSECURE")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false),
            judge_endpoint: std::env::var("LLM_JUDGE_ENDPOINT").ok(),
            judge_model,
            judge_provider,
            judge_api_key: std::env::var("LLM_JUDGE_API_KEY").ok(),
        }
    }
}

fn detect_provider(explicit: &str, model: &str) -> LlmProvider {
    match explicit {
        "anthropic" => LlmProvider::Anthropic,
        "openai_new" => LlmProvider::OpenAINew,
        "openai" => LlmProvider::OpenAI,
        _ => {
            if model.starts_with("claude") {
                LlmProvider::Anthropic
            } else if model.starts_with("gpt-5")
                || model.starts_with("o3")
                || model.starts_with("o4")
            {
                LlmProvider::OpenAINew
            } else {
                LlmProvider::OpenAI
            }
        }
    }
}

/// Ground truth for evaluating LLM responses.
///
/// Fields should contain **human-readable descriptions** (node kind, title,
/// summary) rather than opaque IDs. The judge evaluates semantically.
#[derive(Debug, Clone)]
pub struct EvaluationGroundTruth {
    /// Description of the failure point (e.g. "incident — System incident requiring diagnosis").
    pub expected_failure_point: Option<String>,
    /// Description of the restart node (e.g. "decision node — the first recovery decision").
    pub expected_restart_node: Option<String>,
    /// The expected dominant reason or rationale.
    pub expected_reason: Option<String>,
    /// Short domain label for context (e.g. "operations", "software debugging").
    pub domain_context: Option<String>,
}

/// Result of an LLM evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmEvaluationResult {
    pub llm_response: String,
    pub llm_task_success: bool,
    pub llm_restart_accuracy: bool,
    pub llm_reason_preserved: bool,
    pub llm_latency_ms: f64,
    pub llm_prompt_tokens: u32,
    pub llm_completion_tokens: u32,
    /// Raw judge response for post-hoc analysis. `None` when judge call failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm_judge_raw: Option<String>,
}

/// Call an LLM endpoint and return `(content, prompt_tokens, completion_tokens)`.
///
/// Dispatches to the correct wire format based on [`LlmProvider`].
#[allow(clippy::too_many_arguments)]
pub async fn call_llm(
    client: &reqwest::Client,
    endpoint: &str,
    model: &str,
    provider: LlmProvider,
    api_key: Option<&str>,
    prompt: &str,
    max_tokens: u32,
    temperature: f64,
) -> Result<(String, u32, u32), Box<dyn Error + Send + Sync>> {
    match provider {
        LlmProvider::OpenAI | LlmProvider::OpenAINew => {
            call_openai(
                client,
                endpoint,
                model,
                provider,
                api_key,
                prompt,
                max_tokens,
                temperature,
            )
            .await
        }
        LlmProvider::Anthropic => {
            call_anthropic(client, endpoint, model, api_key, prompt, max_tokens).await
        }
    }
}

// ── OpenAI / OpenAI-new wire format ─────────────────────────────────

#[allow(clippy::too_many_arguments)]
async fn call_openai(
    client: &reqwest::Client,
    endpoint: &str,
    model: &str,
    provider: LlmProvider,
    api_key: Option<&str>,
    prompt: &str,
    max_tokens: u32,
    temperature: f64,
) -> Result<(String, u32, u32), Box<dyn Error + Send + Sync>> {
    let mut body = serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": prompt}],
        "temperature": temperature,
    });

    // vLLM-specific: disable thinking mode for local models only.
    if provider == LlmProvider::OpenAI {
        body["chat_template_kwargs"] = serde_json::json!({"enable_thinking": false});
    }

    // GPT-5.x / o3 / o4 require `max_completion_tokens` instead of `max_tokens`.
    match provider {
        LlmProvider::OpenAINew => {
            body["max_completion_tokens"] = serde_json::json!(max_tokens);
        }
        _ => {
            body["max_tokens"] = serde_json::json!(max_tokens);
        }
    }

    let mut req = client.post(endpoint).json(&body);
    match provider {
        LlmProvider::OpenAINew => {
            // OpenAI-new always requires a bearer token.
            if let Some(key) = api_key {
                req = req.bearer_auth(key);
            }
        }
        _ => {
            if let Some(key) = api_key {
                req = req.bearer_auth(key);
            }
        }
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
        .map(|c| c.message.content.clone())
        .unwrap_or_default();
    let usage = chat.usage.unwrap_or(OpenAiUsage {
        prompt_tokens: 0,
        completion_tokens: 0,
    });
    Ok((content, usage.prompt_tokens, usage.completion_tokens))
}

#[derive(Deserialize)]
struct OpenAiChatResponse {
    choices: Vec<OpenAiChoice>,
    usage: Option<OpenAiUsage>,
}

#[derive(Deserialize)]
struct OpenAiChoice {
    message: OpenAiMessage,
}

#[derive(Deserialize)]
struct OpenAiMessage {
    content: String,
}

#[derive(Deserialize)]
struct OpenAiUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
}

// ── Anthropic wire format ───────────────────────────────────────────

async fn call_anthropic(
    client: &reqwest::Client,
    endpoint: &str,
    model: &str,
    api_key: Option<&str>,
    prompt: &str,
    max_tokens: u32,
) -> Result<(String, u32, u32), Box<dyn Error + Send + Sync>> {
    let body = serde_json::json!({
        "model": model,
        "max_tokens": max_tokens,
        "messages": [{"role": "user", "content": prompt}],
    });

    let mut req = client
        .post(endpoint)
        .header("content-type", "application/json")
        .header("anthropic-version", "2023-06-01")
        .json(&body);

    if let Some(key) = api_key {
        req = req.header("x-api-key", key);
    }

    let response = req.send().await?;
    let status = response.status();
    if !status.is_success() {
        let body_text = response.text().await.unwrap_or_default();
        return Err(format!("LLM request failed with {status}: {body_text}").into());
    }

    let resp: AnthropicResponse = response.json().await?;
    let content = resp
        .content
        .first()
        .map(|b| b.text.clone())
        .unwrap_or_default();
    let prompt_tokens = resp.usage.input_tokens;
    let completion_tokens = resp.usage.output_tokens;
    Ok((content, prompt_tokens, completion_tokens))
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicBlock>,
    usage: AnthropicUsage,
}

#[derive(Deserialize)]
struct AnthropicBlock {
    text: String,
}

#[derive(Deserialize)]
struct AnthropicUsage {
    input_tokens: u32,
    output_tokens: u32,
}

// ── evaluate_with_llm ───────────────────────────────────────────────

/// Evaluate rendered context against ground truth using an LLM.
pub async fn evaluate_with_llm(
    config: &LlmEvaluatorConfig,
    rendered_context: &str,
    question: &str,
    ground_truth: &EvaluationGroundTruth,
) -> Result<LlmEvaluationResult, Box<dyn Error + Send + Sync>> {
    let prompt = PROMPT_CONFIG
        .inference_prompt
        .replace("{rendered_context}", rendered_context)
        .replace("{question}", question);

    let client = build_http_client(config)?;

    let start = std::time::Instant::now();
    let (content, prompt_tokens, completion_tokens) = call_llm(
        &client,
        &config.endpoint,
        &config.model,
        config.provider,
        config.api_key.as_deref(),
        &prompt,
        config.max_tokens,
        config.temperature,
    )
    .await?;
    let latency_ms = start.elapsed().as_secs_f64() * 1000.0;

    // LLM-as-judge: use separate judge endpoint if configured
    let judge_client = if config.judge_endpoint.is_some() {
        build_http_client(config)?
    } else {
        client
    };
    eprintln!("[LLM-EVAL] inference response: {}", content.replace('\n', " ").chars().take(500).collect::<String>());
    let (judge_result, judge_raw) = match judge_response(config, &judge_client, &content, ground_truth).await {
        Ok((verdict, raw)) => {
            eprintln!("[LLM-EVAL] judge verdict: task={} restart={} reason={}", verdict.task_correct, verdict.restart_correct, verdict.reason_preserved);
            (verdict, Some(raw))
        }
        Err(e) => {
            eprintln!("[LLM-EVAL] judge FAILED: {e}");
            (JudgeVerdict {
                task_correct: false,
                restart_correct: false,
                reason_preserved: false,
            }, None)
        }
    };

    Ok(LlmEvaluationResult {
        llm_response: content,
        llm_task_success: judge_result.task_correct,
        llm_restart_accuracy: judge_result.restart_correct,
        llm_reason_preserved: judge_result.reason_preserved,
        llm_latency_ms: latency_ms,
        llm_prompt_tokens: prompt_tokens,
        llm_completion_tokens: completion_tokens,
        llm_judge_raw: judge_raw,
    })
}

/// Evaluate with explicit prompt config and LLM config — no global state.
/// Use this to iterate model×prompt cells within a single process.
pub async fn evaluate_with_config(
    prompts: &PromptConfig,
    config: &LlmEvaluatorConfig,
    rendered_context: &str,
    question: &str,
    ground_truth: &EvaluationGroundTruth,
) -> Result<LlmEvaluationResult, Box<dyn Error + Send + Sync>> {
    let prompt = prompts
        .inference_prompt
        .replace("{rendered_context}", rendered_context)
        .replace("{question}", question);

    let client = build_http_client(config)?;

    let start = std::time::Instant::now();
    let (content, prompt_tokens, completion_tokens) = call_llm(
        &client,
        &config.endpoint,
        &config.model,
        config.provider,
        config.api_key.as_deref(),
        &prompt,
        config.max_tokens,
        config.temperature,
    )
    .await?;
    let latency_ms = start.elapsed().as_secs_f64() * 1000.0;

    let judge_client = if config.judge_endpoint.is_some() {
        build_http_client(config)?
    } else {
        client
    };
    eprintln!("[LLM-EVAL] inference response: {}", content.replace('\n', " ").chars().take(500).collect::<String>());
    let (judge_result, judge_raw) = match judge_response_with_prompts(prompts, config, &judge_client, &content, ground_truth).await {
        Ok((verdict, raw)) => {
            eprintln!("[LLM-EVAL] judge verdict: task={} restart={} reason={}", verdict.task_correct, verdict.restart_correct, verdict.reason_preserved);
            (verdict, Some(raw))
        }
        Err(e) => {
            eprintln!("[LLM-EVAL] judge FAILED: {e}");
            (JudgeVerdict { task_correct: false, restart_correct: false, reason_preserved: false }, None)
        }
    };

    Ok(LlmEvaluationResult {
        llm_response: content,
        llm_task_success: judge_result.task_correct,
        llm_restart_accuracy: judge_result.restart_correct,
        llm_reason_preserved: judge_result.reason_preserved,
        llm_latency_ms: latency_ms,
        llm_prompt_tokens: prompt_tokens,
        llm_completion_tokens: completion_tokens,
        llm_judge_raw: judge_raw,
    })
}

// ── judge ───────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct JudgeVerdict {
    task_correct: bool,
    restart_correct: bool,
    reason_preserved: bool,
}

async fn judge_response(
    config: &LlmEvaluatorConfig,
    client: &reqwest::Client,
    llm_response: &str,
    ground_truth: &EvaluationGroundTruth,
) -> Result<(JudgeVerdict, String), Box<dyn Error + Send + Sync>> {
    judge_response_with_prompts(&PROMPT_CONFIG, config, client, llm_response, ground_truth).await
}

async fn judge_response_with_prompts(
    prompts: &PromptConfig,
    config: &LlmEvaluatorConfig,
    client: &reqwest::Client,
    llm_response: &str,
    ground_truth: &EvaluationGroundTruth,
) -> Result<(JudgeVerdict, String), Box<dyn Error + Send + Sync>> {
    let expected_failure = ground_truth
        .expected_failure_point
        .as_deref()
        .unwrap_or("not specified");
    let expected_restart = ground_truth
        .expected_restart_node
        .as_deref()
        .unwrap_or("not specified");
    let expected_reason = ground_truth
        .expected_reason
        .as_deref()
        .unwrap_or("not specified");
    let domain_ctx = ground_truth
        .domain_context
        .as_deref()
        .unwrap_or("operational");

    let judge_prompt = prompts
        .judge_prompt
        .replace("{domain_context}", domain_ctx)
        .replace("{llm_response}", llm_response)
        .replace("{expected_failure}", expected_failure)
        .replace("{expected_restart}", expected_restart)
        .replace("{expected_reason}", expected_reason);

    let judge_endpoint = config.judge_endpoint.as_deref().unwrap_or(&config.endpoint);
    let judge_model = config.judge_model.as_deref().unwrap_or(&config.model);
    let judge_provider = config.judge_provider.unwrap_or(config.provider);
    let judge_api_key = config
        .judge_api_key
        .as_deref()
        .or(config.api_key.as_deref());

    let (judge_content, _, _) = call_llm(
        client,
        judge_endpoint,
        judge_model,
        judge_provider,
        judge_api_key,
        &judge_prompt,
        prompts.judge_max_tokens,
        0.0,
    )
    .await?;

    // Parse JSON from the judge response -- handle potential markdown wrapping
    let json_str = judge_content
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let verdict: JudgeVerdict = serde_json::from_str(json_str)
        .map_err(|e| format!("failed to parse judge response '{judge_content}': {e}"))?;
    Ok((verdict, judge_content))
}

fn build_http_client(
    config: &LlmEvaluatorConfig,
) -> Result<reqwest::Client, Box<dyn Error + Send + Sync>> {
    let mut builder = reqwest::Client::builder();

    if config.tls_insecure {
        builder = builder.danger_accept_invalid_certs(true);
    }

    if let (Some(cert_path), Some(key_path)) = (&config.tls_cert_path, &config.tls_key_path) {
        let cert_pem = std::fs::read(cert_path)?;
        let key_pem = std::fs::read(key_path)?;
        let mut combined = cert_pem;
        combined.extend_from_slice(&key_pem);
        let identity = reqwest::Identity::from_pem(&combined)?;
        builder = builder.identity(identity);
    }

    Ok(builder.build()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_from_env_uses_defaults() {
        let config = LlmEvaluatorConfig::from_env();
        assert!(config.endpoint.contains("chat/completions"));
        assert!(!config.model.is_empty());
        assert!(config.max_tokens > 0);
        // Default provider for Qwen model is OpenAI-compatible
        assert_eq!(config.provider, LlmProvider::OpenAI);
        // No API key set by default
        assert!(config.api_key.is_none());
        // Judge fields fall back to None
        assert!(config.judge_provider.is_none());
        assert!(config.judge_api_key.is_none());
    }

    #[test]
    fn evaluation_result_serializes_to_json() {
        let result = LlmEvaluationResult {
            llm_response: "test".to_string(),
            llm_task_success: true,
            llm_restart_accuracy: true,
            llm_reason_preserved: false,
            llm_latency_ms: 123.4,
            llm_prompt_tokens: 50,
            llm_completion_tokens: 30,
            llm_judge_raw: Some("{\"task_correct\":true}".to_string()),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("llm_task_success"));
        assert!(json.contains("llm_judge_raw"));
    }
}
