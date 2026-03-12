use std::io;

use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::json;

use crate::logging::debug_log_value;
use crate::openai_compat_client::parse_json_only;

const ANTHROPIC_VERSION: &str = "2023-06-01";

#[derive(Debug, Clone)]
pub struct AnthropicClient {
    http_client: reqwest::Client,
    base_url: String,
    model: String,
}

impl AnthropicClient {
    pub fn from_env() -> io::Result<Self> {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    pub(crate) fn from_lookup<F>(lookup: F) -> io::Result<Self>
    where
        F: Fn(&str) -> Option<String>,
    {
        let api_key = lookup_required(&lookup, "ANTHROPIC_API_KEY")?;
        let model = lookup_required(&lookup, "ANTHROPIC_MODEL")?;
        let base_url =
            lookup("ANTHROPIC_BASE_URL").unwrap_or_else(|| "https://api.anthropic.com".to_string());
        Self::new(base_url, model, api_key)
    }

    pub fn new(
        base_url: impl Into<String>,
        model: impl Into<String>,
        api_key: String,
    ) -> io::Result<Self> {
        let base_url = normalize_base_url(&base_url.into())?;
        let http_client = build_http_client(&api_key)?;
        debug_log_value("anthropic base_url", &base_url);
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
        let endpoint = format!("{}/v1/messages", self.base_url);
        debug_log_value("anthropic request", &endpoint);
        let response = self
            .http_client
            .post(endpoint)
            .json(&json!({
                "model": self.model,
                "max_tokens": 1200,
                "temperature": 0,
                "system": system_prompt,
                "messages": [
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
                "anthropic request failed: {status} {body}"
            )));
        }

        let response: AnthropicMessagesResponse =
            response.json().await.map_err(io::Error::other)?;
        let content = response
            .content
            .iter()
            .find(|block| block.kind == "text")
            .map(|block| block.text.as_str())
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "missing Anthropic text content")
            })?;
        debug_log_value("anthropic response bytes", content.len());
        parse_json_only(content)
    }
}

#[derive(Debug, Deserialize)]
struct AnthropicMessagesResponse {
    content: Vec<AnthropicContentBlock>,
}

#[derive(Debug, Deserialize)]
struct AnthropicContentBlock {
    #[serde(rename = "type")]
    kind: String,
    text: String,
}

fn normalize_base_url(base_url: &str) -> io::Result<String> {
    let base_url = base_url.trim_end_matches('/');
    if !(base_url.starts_with("http://") || base_url.starts_with("https://")) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Anthropic base url must start with http:// or https://",
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

fn build_http_client(api_key: &str) -> io::Result<reqwest::Client> {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(
        "x-api-key",
        HeaderValue::from_str(api_key).map_err(io::Error::other)?,
    );
    headers.insert(
        "anthropic-version",
        HeaderValue::from_static(ANTHROPIC_VERSION),
    );

    reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, VecDeque};
    use std::sync::Arc;

    use serde::Deserialize;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::sync::Mutex;

    use super::AnthropicClient;

    #[derive(Debug, Deserialize, PartialEq, Eq)]
    struct Selection {
        selected_step_node_id: String,
    }

    #[tokio::test]
    async fn chat_json_parses_anthropic_text_content() {
        let base_url = spawn_http_server(vec![(
            200,
            r#"{"content":[{"type":"text","text":"{\"selected_step_node_id\":\"node:work_item:anthropic\"}"}]}"#
                .to_string(),
        )])
        .await;
        let client = AnthropicClient::new(base_url, "claude-demo", "secret".to_string())
            .expect("client should build");

        let parsed: Selection = client
            .chat_json("system", "user")
            .await
            .expect("response should parse");

        assert_eq!(parsed.selected_step_node_id, "node:work_item:anthropic");
    }

    #[tokio::test]
    async fn chat_json_reports_http_failures() {
        let base_url =
            spawn_http_server(vec![(429, r#"{"error":"rate_limited"}"#.to_string())]).await;
        let client = AnthropicClient::new(base_url, "claude-demo", "secret".to_string())
            .expect("client should build");

        let error = client
            .chat_json::<Selection>("system", "user")
            .await
            .expect_err("http failure should surface");

        assert!(error.to_string().contains("anthropic request failed"));
    }

    #[test]
    fn from_lookup_uses_default_base_url() {
        let env = BTreeMap::from([
            ("ANTHROPIC_API_KEY".to_string(), "secret".to_string()),
            ("ANTHROPIC_MODEL".to_string(), "claude-3-7".to_string()),
        ]);

        let client = AnthropicClient::from_lookup(|key| env.get(key).cloned())
            .expect("lookup config should load");

        assert_eq!(client.base_url, "https://api.anthropic.com");
        assert_eq!(client.model, "claude-3-7");
    }

    #[test]
    fn new_rejects_invalid_base_url() {
        let error = AnthropicClient::new("anthropic.local", "claude", "secret".to_string())
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
