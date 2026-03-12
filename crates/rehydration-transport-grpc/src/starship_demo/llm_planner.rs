use std::io;

use serde::de::DeserializeOwned;

use crate::starship_demo::anthropic_client::AnthropicClient;
use crate::starship_demo::openai_compat_client::{OpenAiCompatClient, OpenAiCompatMode};

#[derive(Clone)]
pub enum LlmPlanner {
    OpenAiCompat(OpenAiCompatClient),
    Anthropic(AnthropicClient),
}

impl LlmPlanner {
    pub fn from_env() -> io::Result<Self> {
        match std::env::var("LLM_PROVIDER")
            .unwrap_or_else(|_| "vllm".to_string())
            .to_lowercase()
            .as_str()
        {
            "vllm" => Ok(Self::OpenAiCompat(OpenAiCompatClient::from_env(
                OpenAiCompatMode::Vllm,
            )?)),
            "openai" => Ok(Self::OpenAiCompat(OpenAiCompatClient::from_env(
                OpenAiCompatMode::OpenAi,
            )?)),
            "openai_compat" | "openai-compatible" => Ok(Self::OpenAiCompat(
                OpenAiCompatClient::from_env(OpenAiCompatMode::Custom)?,
            )),
            "anthropic" | "claude" => Ok(Self::Anthropic(AnthropicClient::from_env()?)),
            other => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unsupported LLM_PROVIDER `{other}`"),
            )),
        }
    }

    pub async fn chat_json<T>(&self, system_prompt: &str, user_prompt: &str) -> io::Result<T>
    where
        T: DeserializeOwned,
    {
        match self {
            Self::OpenAiCompat(client) => client.chat_json(system_prompt, user_prompt).await,
            Self::Anthropic(client) => client.chat_json(system_prompt, user_prompt).await,
        }
    }
}
