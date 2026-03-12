use std::io;

use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::json;

use crate::agentic_reference::debug_log_value;
use crate::starship_demo::openai_compat_client::parse_json_only;

const ANTHROPIC_VERSION: &str = "2023-06-01";

#[derive(Clone)]
pub struct AnthropicClient {
    http_client: reqwest::Client,
    base_url: String,
    model: String,
}

impl AnthropicClient {
    pub fn from_env() -> io::Result<Self> {
        let api_key = std::env::var("ANTHROPIC_API_KEY").map_err(|_| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "missing ANTHROPIC_API_KEY environment variable",
            )
        })?;
        let model = std::env::var("ANTHROPIC_MODEL").map_err(|_| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "missing ANTHROPIC_MODEL environment variable",
            )
        })?;
        let base_url = std::env::var("ANTHROPIC_BASE_URL")
            .unwrap_or_else(|_| "https://api.anthropic.com".to_string());
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
