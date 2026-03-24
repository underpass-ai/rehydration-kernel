//! LLM-in-the-loop evaluator for paper use cases.
//!
//! Sends rehydrated rendered context to an OpenAI-compatible endpoint
//! (e.g. vLLM) and evaluates the response against ground truth.

use std::collections::HashMap;
use std::error::Error;

use serde::{Deserialize, Serialize};

/// Configuration for the LLM evaluator.
#[derive(Debug, Clone)]
pub struct LlmEvaluatorConfig {
    /// OpenAI-compatible chat completions endpoint.
    /// e.g. `https://llm.underpassai.com/v1/chat/completions`
    pub endpoint: String,
    /// Model name to use.
    pub model: String,
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
}

impl LlmEvaluatorConfig {
    pub fn from_env() -> Self {
        Self {
            endpoint: std::env::var("LLM_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:8000/v1/chat/completions".to_string()),
            model: std::env::var("LLM_MODEL").unwrap_or_else(|_| "Qwen/Qwen3-8B".to_string()),
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
        }
    }
}

/// Ground truth for evaluating LLM responses.
#[derive(Debug, Clone)]
pub struct EvaluationGroundTruth {
    /// The expected failure point or root cause node.
    pub expected_failure_point: Option<String>,
    /// The expected restart/rehydration node.
    pub expected_restart_node: Option<String>,
    /// The expected dominant reason text (substring match).
    pub expected_reason: Option<String>,
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
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    max_tokens: u32,
    temperature: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    chat_template_kwargs: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
    usage: Option<ChatUsage>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatChoiceMessage,
}

#[derive(Deserialize)]
struct ChatChoiceMessage {
    content: String,
}

#[derive(Deserialize)]
struct ChatUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
}

/// Evaluate rendered context against ground truth using an LLM.
pub async fn evaluate_with_llm(
    config: &LlmEvaluatorConfig,
    rendered_context: &str,
    question: &str,
    ground_truth: &EvaluationGroundTruth,
) -> Result<LlmEvaluationResult, Box<dyn Error + Send + Sync>> {
    let prompt = format!(
        "Given this rehydrated context from an operational graph:\n\n{rendered_context}\n\n\
         Question: {question}\n\n\
         Answer concisely in JSON with these fields:\n\
         {{\"failure_point\": \"the node or event that caused the issue\",\n\
         \"restart_node\": \"the node from which the system should restart\",\n\
         \"reason\": \"the dominant reason or rationale\"}}"
    );

    let mut disable_thinking = HashMap::new();
    disable_thinking.insert(
        "enable_thinking".to_string(),
        serde_json::Value::Bool(false),
    );

    let request = ChatRequest {
        model: config.model.clone(),
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: prompt,
        }],
        max_tokens: config.max_tokens,
        temperature: config.temperature,
        chat_template_kwargs: Some(disable_thinking),
    };

    let client = build_http_client(config)?;

    let start = std::time::Instant::now();
    let response = client.post(&config.endpoint).json(&request).send().await?;
    let latency_ms = start.elapsed().as_secs_f64() * 1000.0;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(format!("LLM request failed with {status}: {body}").into());
    }

    let chat_response: ChatResponse = response.json().await?;
    let content = chat_response
        .choices
        .first()
        .map(|c| c.message.content.clone())
        .unwrap_or_default();
    let usage = chat_response.usage.unwrap_or(ChatUsage {
        prompt_tokens: 0,
        completion_tokens: 0,
    });

    // LLM-as-judge: ask the same model to evaluate the response
    let judge_result = judge_response(config, &client, &content, ground_truth)
        .await
        .unwrap_or(JudgeVerdict {
            task_correct: false,
            restart_correct: false,
            reason_preserved: false,
        });

    Ok(LlmEvaluationResult {
        llm_response: content,
        llm_task_success: judge_result.task_correct,
        llm_restart_accuracy: judge_result.restart_correct,
        llm_reason_preserved: judge_result.reason_preserved,
        llm_latency_ms: latency_ms,
        llm_prompt_tokens: usage.prompt_tokens,
        llm_completion_tokens: usage.completion_tokens,
    })
}

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
) -> Result<JudgeVerdict, Box<dyn Error + Send + Sync>> {
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

    let judge_prompt = format!(
        "You are an evaluation judge. Compare the LLM response against the ground truth.\n\n\
         LLM Response:\n{llm_response}\n\n\
         Ground Truth:\n\
         - Expected failure point: {expected_failure}\n\
         - Expected restart node: {expected_restart}\n\
         - Expected reason/rationale: {expected_reason}\n\n\
         Evaluate semantically (not exact string match). \
         The response is correct if it identifies the same concept, \
         even with different wording.\n\n\
         Answer ONLY with JSON, no other text:\n\
         {{\"task_correct\": true/false, \"restart_correct\": true/false, \"reason_preserved\": true/false}}"
    );

    let mut disable_thinking = HashMap::new();
    disable_thinking.insert(
        "enable_thinking".to_string(),
        serde_json::Value::Bool(false),
    );

    let request = ChatRequest {
        model: config.model.clone(),
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: judge_prompt,
        }],
        max_tokens: 50,
        temperature: 0.0,
        chat_template_kwargs: Some(disable_thinking),
    };

    let response = client.post(&config.endpoint).json(&request).send().await?;
    if !response.status().is_success() {
        return Err("judge request failed".into());
    }

    let chat_response: ChatResponse = response.json().await?;
    let judge_content = chat_response
        .choices
        .first()
        .map(|c| c.message.content.clone())
        .unwrap_or_default();

    // Parse JSON from the judge response — handle potential markdown wrapping
    let json_str = judge_content
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    serde_json::from_str(json_str)
        .map_err(|e| format!("failed to parse judge response '{judge_content}': {e}").into())
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
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("llm_task_success"));
    }
}
