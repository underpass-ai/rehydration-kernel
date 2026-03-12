use std::io;

use serde::de::DeserializeOwned;

use crate::starship_demo::anthropic_client::AnthropicClient;
use crate::starship_demo::openai_compat_client::{OpenAiCompatClient, OpenAiCompatMode};

#[derive(Debug, Clone)]
pub enum LlmPlanner {
    OpenAiCompat(OpenAiCompatClient),
    Anthropic(AnthropicClient),
}

impl LlmPlanner {
    pub fn from_env() -> io::Result<Self> {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    fn from_lookup<F>(lookup: F) -> io::Result<Self>
    where
        F: Fn(&str) -> Option<String>,
    {
        match lookup("LLM_PROVIDER")
            .unwrap_or_else(|| "vllm".to_string())
            .to_lowercase()
            .as_str()
        {
            "vllm" => Ok(Self::OpenAiCompat(OpenAiCompatClient::from_lookup(
                OpenAiCompatMode::Vllm,
                &lookup,
            )?)),
            "openai" => Ok(Self::OpenAiCompat(OpenAiCompatClient::from_lookup(
                OpenAiCompatMode::OpenAi,
                &lookup,
            )?)),
            "openai_compat" | "openai-compatible" => Ok(Self::OpenAiCompat(
                OpenAiCompatClient::from_lookup(OpenAiCompatMode::Custom, &lookup)?,
            )),
            "anthropic" | "claude" => Ok(Self::Anthropic(AnthropicClient::from_lookup(&lookup)?)),
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::LlmPlanner;

    #[test]
    fn from_lookup_selects_openai_compat_and_anthropic_variants() {
        let vllm_env = BTreeMap::from([
            ("LLM_PROVIDER".to_string(), "vllm".to_string()),
            ("VLLM_BASE_URL".to_string(), "http://vllm".to_string()),
            ("VLLM_MODEL".to_string(), "qwen".to_string()),
        ]);
        assert!(matches!(
            LlmPlanner::from_lookup(|key| vllm_env.get(key).cloned()),
            Ok(LlmPlanner::OpenAiCompat(_))
        ));

        let anthropic_env = BTreeMap::from([
            ("LLM_PROVIDER".to_string(), "anthropic".to_string()),
            ("ANTHROPIC_API_KEY".to_string(), "secret".to_string()),
            ("ANTHROPIC_MODEL".to_string(), "claude-3-7".to_string()),
        ]);
        assert!(matches!(
            LlmPlanner::from_lookup(|key| anthropic_env.get(key).cloned()),
            Ok(LlmPlanner::Anthropic(_))
        ));
    }

    #[test]
    fn from_lookup_rejects_unknown_provider() {
        let env = BTreeMap::from([("LLM_PROVIDER".to_string(), "mystery".to_string())]);

        let error = LlmPlanner::from_lookup(|key| env.get(key).cloned())
            .expect_err("unknown providers must fail");

        assert!(error.to_string().contains("unsupported LLM_PROVIDER"));
    }
}
