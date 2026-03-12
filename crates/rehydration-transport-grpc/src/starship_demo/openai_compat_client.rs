use std::io;

use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::json;

use crate::agentic_reference::{debug_log, debug_log_value};

#[derive(Clone)]
pub struct OpenAiCompatClient {
    http_client: reqwest::Client,
    base_url: String,
    model: String,
}

impl OpenAiCompatClient {
    pub fn from_env(mode: OpenAiCompatMode) -> io::Result<Self> {
        match mode {
            OpenAiCompatMode::Vllm => {
                let base_url = std::env::var("VLLM_BASE_URL").map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::NotFound,
                        "missing VLLM_BASE_URL environment variable",
                    )
                })?;
                let model = std::env::var("VLLM_MODEL").map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::NotFound,
                        "missing VLLM_MODEL environment variable",
                    )
                })?;
                let api_key = std::env::var("VLLM_API_KEY").ok();
                Self::new(base_url, model, api_key)
            }
            OpenAiCompatMode::OpenAi => {
                let model = std::env::var("OPENAI_MODEL").map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::NotFound,
                        "missing OPENAI_MODEL environment variable",
                    )
                })?;
                let api_key = std::env::var("OPENAI_API_KEY").map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::NotFound,
                        "missing OPENAI_API_KEY environment variable",
                    )
                })?;
                let base_url = std::env::var("OPENAI_BASE_URL")
                    .unwrap_or_else(|_| "https://api.openai.com".to_string());
                Self::new(base_url, model, Some(api_key))
            }
            OpenAiCompatMode::Custom => {
                let base_url = std::env::var("OPENAI_COMPAT_BASE_URL")
                    .or_else(|_| std::env::var("OPENAI_BASE_URL"))
                    .map_err(|_| {
                        io::Error::new(
                            io::ErrorKind::NotFound,
                            "missing OPENAI_COMPAT_BASE_URL or OPENAI_BASE_URL environment variable",
                        )
                    })?;
                let model = std::env::var("OPENAI_MODEL").map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::NotFound,
                        "missing OPENAI_MODEL environment variable",
                    )
                })?;
                let api_key = std::env::var("OPENAI_API_KEY")
                    .or_else(|_| std::env::var("OPENAI_COMPAT_API_KEY"))
                    .ok();
                Self::new(base_url, model, api_key)
            }
        }
    }

    pub fn new(
        base_url: impl Into<String>,
        model: impl Into<String>,
        api_key: Option<String>,
    ) -> io::Result<Self> {
        let base_url = normalize_base_url(&base_url.into())?;
        let http_client = build_http_client(api_key)?;
        debug_log_value("openai-compatible base_url", &base_url);
        Ok(Self {
            http_client,
            base_url,
            model: model.into(),
        })
    }

    pub async fn chat_json<T>(&self, system_prompt: &str, user_prompt: &str) -> io::Result<T>
    where
        T: DeserializeOwned,
    {
        let content = self
            .chat_completion_content(system_prompt, user_prompt)
            .await?;
        debug_log_value("openai-compatible response bytes", content.len());

        if let Ok(value) = parse_json_only(&content) {
            return Ok(value);
        }

        debug_log_value("openai-compatible invalid content", &content);
        let repaired_prompt = format!(
            "{user_prompt}\n\nYour previous answer was invalid because it was not a usable JSON object. Return exactly one compact JSON object matching the requested schema. Never return null, prose, markdown, or code fences."
        );
        let repaired_content = self
            .chat_completion_content(system_prompt, &repaired_prompt)
            .await?;
        debug_log_value(
            "openai-compatible repaired response bytes",
            repaired_content.len(),
        );
        parse_json_only(&repaired_content)
    }

    async fn chat_completion_content(
        &self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> io::Result<String> {
        let endpoint = format!("{}/v1/chat/completions", self.base_url);
        debug_log_value("openai-compatible request", &endpoint);
        let response = self
            .http_client
            .post(endpoint)
            .json(&json!({
                "model": self.model,
                "temperature": 0,
                "max_tokens": 1400,
                "response_format": { "type": "json_object" },
                "messages": [
                    { "role": "system", "content": system_prompt },
                    { "role": "user", "content": user_prompt }
                ]
            }))
            .send()
            .await
            .map_err(io::Error::other)?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(io::Error::other(format!(
                "openai-compatible request failed: {status} {body}"
            )));
        }

        let response: ChatCompletionResponse = response.json().await.map_err(io::Error::other)?;
        let content = response
            .choices
            .first()
            .and_then(|choice| choice.message.content.clone())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "missing OpenAI-compatible response content",
                )
            })?;
        Ok(content)
    }
}

#[derive(Clone, Copy)]
pub enum OpenAiCompatMode {
    Vllm,
    OpenAi,
    Custom,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    content: Option<String>,
}

fn normalize_base_url(base_url: &str) -> io::Result<String> {
    let base_url = base_url.trim_end_matches('/');
    if !(base_url.starts_with("http://") || base_url.starts_with("https://")) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "OpenAI-compatible base url must start with http:// or https://",
        ));
    }
    Ok(base_url.to_string())
}

fn build_http_client(api_key: Option<String>) -> io::Result<reqwest::Client> {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    if let Some(token) = api_key {
        let header = HeaderValue::from_str(&format!("Bearer {token}")).map_err(io::Error::other)?;
        headers.insert(AUTHORIZATION, header);
    }

    reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .map_err(io::Error::other)
}

pub fn parse_json_only<T>(content: &str) -> io::Result<T>
where
    T: DeserializeOwned,
{
    let sanitized = strip_thinking_blocks(content);
    let trimmed = sanitized.trim();
    if let Ok(value) = serde_json::from_str::<T>(trimmed) {
        return Ok(value);
    }

    if let Some(fenced) = trimmed
        .strip_prefix("```json")
        .and_then(|value| value.strip_suffix("```"))
        .map(str::trim)
        .or_else(|| {
            trimmed
                .strip_prefix("```")
                .and_then(|value| value.strip_suffix("```"))
                .map(str::trim)
        })
        && let Ok(value) = serde_json::from_str::<T>(fenced)
    {
        return Ok(value);
    }

    if let Some(start) = trimmed.find('{')
        && let Some(end) = trimmed.rfind('}')
    {
        let slice = &trimmed[start..=end];
        if let Ok(value) = serde_json::from_str::<T>(slice) {
            return Ok(value);
        }
    }

    debug_log("failed to parse JSON-only response from OpenAI-compatible endpoint");
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "OpenAI-compatible response was not valid JSON",
    ))
}

fn strip_thinking_blocks(content: &str) -> String {
    let mut remaining = content.trim();
    let mut sanitized = String::new();

    loop {
        if !remaining.starts_with("<think>") {
            sanitized.push_str(remaining);
            break;
        }

        let Some(end) = remaining.find("</think>") else {
            sanitized.push_str(remaining);
            break;
        };

        remaining = remaining[end + "</think>".len()..].trim_start();
    }

    sanitized
}

#[cfg(test)]
mod tests {
    use super::parse_json_only;
    use serde::Deserialize;

    #[derive(Debug, Deserialize, PartialEq, Eq)]
    struct Selection {
        selected_step_node_id: String,
    }

    #[test]
    fn parse_json_only_accepts_plain_json() {
        let parsed: Selection =
            parse_json_only("{\"selected_step_node_id\":\"node:work_item:one\"}")
                .expect("plain JSON should parse");
        assert_eq!(
            parsed,
            Selection {
                selected_step_node_id: "node:work_item:one".to_string(),
            }
        );
    }

    #[test]
    fn parse_json_only_strips_thinking_block_before_json() {
        let content = r#"
<think>
I should choose the in-progress step.
</think>

{"selected_step_node_id":"node:work_item:two"}
"#;

        let parsed: Selection =
            parse_json_only(content).expect("thinking blocks should be stripped before parsing");
        assert_eq!(
            parsed,
            Selection {
                selected_step_node_id: "node:work_item:two".to_string(),
            }
        );
    }
}
