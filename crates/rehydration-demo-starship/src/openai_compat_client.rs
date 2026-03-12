use std::io;

use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::json;

use crate::logging::{debug_log, debug_log_value};

#[derive(Debug, Clone)]
pub struct OpenAiCompatClient {
    http_client: reqwest::Client,
    base_url: String,
    model: String,
}

impl OpenAiCompatClient {
    pub fn from_env(mode: OpenAiCompatMode) -> io::Result<Self> {
        Self::from_lookup(mode, |key| std::env::var(key).ok())
    }

    pub(crate) fn from_lookup<F>(mode: OpenAiCompatMode, lookup: F) -> io::Result<Self>
    where
        F: Fn(&str) -> Option<String>,
    {
        match mode {
            OpenAiCompatMode::Vllm => {
                let base_url = lookup_required(&lookup, "VLLM_BASE_URL")?;
                let model = lookup_required(&lookup, "VLLM_MODEL")?;
                let api_key = lookup("VLLM_API_KEY");
                Self::new(base_url, model, api_key)
            }
            OpenAiCompatMode::OpenAi => {
                let model = lookup_required(&lookup, "OPENAI_MODEL")?;
                let api_key = lookup_required(&lookup, "OPENAI_API_KEY")?;
                let base_url = lookup("OPENAI_BASE_URL")
                    .unwrap_or_else(|| "https://api.openai.com".to_string());
                Self::new(base_url, model, Some(api_key))
            }
            OpenAiCompatMode::Custom => {
                let base_url = lookup("OPENAI_COMPAT_BASE_URL")
                    .or_else(|| lookup("OPENAI_BASE_URL"))
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::NotFound,
                            "missing OPENAI_COMPAT_BASE_URL or OPENAI_BASE_URL environment variable",
                        )
                    })?;
                let model = lookup_required(&lookup, "OPENAI_MODEL")?;
                let api_key = lookup("OPENAI_API_KEY").or_else(|| lookup("OPENAI_COMPAT_API_KEY"));
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

fn lookup_required<F>(lookup: &F, key: &str) -> io::Result<String>
where
    F: Fn(&str) -> Option<String>,
{
    lookup(key).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("missing {key} environment variable"),
        )
    })
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
    use std::collections::{BTreeMap, VecDeque};
    use std::sync::Arc;

    use super::parse_json_only;
    use super::{OpenAiCompatClient, OpenAiCompatMode};
    use serde::Deserialize;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::sync::Mutex;

    #[derive(Debug, Deserialize, PartialEq, Eq)]
    struct Selection {
        selected_step_node_id: String,
    }

    #[tokio::test]
    async fn chat_json_parses_openai_compatible_response() {
        let base_url = spawn_http_server(vec![(
            200,
            r#"{"choices":[{"message":{"content":"{\"selected_step_node_id\":\"node:work_item:one\"}"}}]}"#
                .to_string(),
        )])
        .await;
        let client =
            OpenAiCompatClient::new(base_url, "demo-model", None).expect("client should build");

        let parsed: Selection = client
            .chat_json("system", "user")
            .await
            .expect("response should parse");

        assert_eq!(parsed.selected_step_node_id, "node:work_item:one");
    }

    #[tokio::test]
    async fn chat_json_repairs_invalid_json_response() {
        let base_url = spawn_http_server(vec![
            (200, r#"{"choices":[{"message":{"content":"not-json"}}]}"#.to_string()),
            (
                200,
                r#"{"choices":[{"message":{"content":"{\"selected_step_node_id\":\"node:work_item:two\"}"}}]}"#
                    .to_string(),
            ),
        ])
        .await;
        let client = OpenAiCompatClient::new(base_url, "demo-model", Some("token".to_string()))
            .expect("client should build");

        let parsed: Selection = client
            .chat_json("system", "user")
            .await
            .expect("repair path should parse");

        assert_eq!(parsed.selected_step_node_id, "node:work_item:two");
    }

    #[tokio::test]
    async fn chat_json_reports_http_failures() {
        let base_url = spawn_http_server(vec![(500, r#"{"error":"boom"}"#.to_string())]).await;
        let client =
            OpenAiCompatClient::new(base_url, "demo-model", None).expect("client should build");

        let error = client
            .chat_json::<Selection>("system", "user")
            .await
            .expect_err("http failure should surface");

        assert!(
            error
                .to_string()
                .contains("openai-compatible request failed")
        );
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

    #[test]
    fn parse_json_only_accepts_fenced_json() {
        let content = "```json\n{\"selected_step_node_id\":\"node:work_item:fenced\"}\n```";

        let parsed: Selection = parse_json_only(content).expect("fenced JSON should parse");

        assert_eq!(parsed.selected_step_node_id, "node:work_item:fenced");
    }

    #[test]
    fn parse_json_only_extracts_embedded_json_object() {
        let content = "prefix {\"selected_step_node_id\":\"node:work_item:embedded\"} suffix";

        let parsed: Selection = parse_json_only(content).expect("embedded JSON should parse");

        assert_eq!(parsed.selected_step_node_id, "node:work_item:embedded");
    }

    #[test]
    fn parse_json_only_rejects_invalid_json() {
        let error =
            parse_json_only::<Selection>("still not json").expect_err("invalid JSON must fail");

        assert!(error.to_string().contains("not valid JSON"));
    }

    #[test]
    fn from_lookup_supports_all_openai_compat_modes() {
        let vllm_env = BTreeMap::from([
            ("VLLM_BASE_URL".to_string(), "http://vllm".to_string()),
            ("VLLM_MODEL".to_string(), "qwen".to_string()),
        ]);
        let vllm = OpenAiCompatClient::from_lookup(OpenAiCompatMode::Vllm, |key| {
            vllm_env.get(key).cloned()
        })
        .expect("vllm config should load");
        assert_eq!(vllm.base_url, "http://vllm");
        assert_eq!(vllm.model, "qwen");

        let openai_env = BTreeMap::from([
            ("OPENAI_API_KEY".to_string(), "key".to_string()),
            ("OPENAI_MODEL".to_string(), "gpt-5".to_string()),
        ]);
        let openai = OpenAiCompatClient::from_lookup(OpenAiCompatMode::OpenAi, |key| {
            openai_env.get(key).cloned()
        })
        .expect("openai config should load");
        assert_eq!(openai.base_url, "https://api.openai.com");

        let custom_env = BTreeMap::from([
            (
                "OPENAI_COMPAT_BASE_URL".to_string(),
                "https://compat.example".to_string(),
            ),
            ("OPENAI_MODEL".to_string(), "compat-model".to_string()),
        ]);
        let custom = OpenAiCompatClient::from_lookup(OpenAiCompatMode::Custom, |key| {
            custom_env.get(key).cloned()
        })
        .expect("custom config should load");
        assert_eq!(custom.base_url, "https://compat.example");
    }

    #[test]
    fn new_rejects_invalid_base_url() {
        let error = OpenAiCompatClient::new("localhost:8000", "model", None)
            .expect_err("base url must be validated");

        assert!(
            error
                .to_string()
                .contains("must start with http:// or https://")
        );
    }

    async fn spawn_http_server(responses: Vec<(u16, String)>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let address = listener.local_addr().expect("listener should have address");
        let responses = Arc::new(Mutex::new(VecDeque::from(responses)));

        tokio::spawn({
            let responses = Arc::clone(&responses);
            async move {
                loop {
                    let (mut socket, _) =
                        listener.accept().await.expect("connection should arrive");
                    let responses = Arc::clone(&responses);
                    tokio::spawn(async move {
                        let mut buffer = vec![0; 65_536];
                        let _ = socket.read(&mut buffer).await.expect("request should read");
                        let (status, body) = responses
                            .lock()
                            .await
                            .pop_front()
                            .unwrap_or((500, r#"{"error":"missing test response"}"#.to_string()));
                        let reason = if status == 200 { "OK" } else { "ERROR" };
                        let response = format!(
                            "HTTP/1.1 {status} {reason}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                            body.len()
                        );
                        socket
                            .write_all(response.as_bytes())
                            .await
                            .expect("response should write");
                    });
                }
            }
        });

        format!("http://{address}")
    }
}
