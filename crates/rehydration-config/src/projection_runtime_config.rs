use std::env;

use crate::env_bool::parse_bool_env;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionRuntimeConfig {
    pub nats_url: String,
    pub enabled: bool,
    pub runtime_state_uri: String,
}

impl ProjectionRuntimeConfig {
    pub fn from_env() -> Self {
        Self {
            nats_url: env::var("NATS_URL").unwrap_or_else(|_| "nats://nats:4222".to_string()),
            enabled: parse_bool_env("ENABLE_PROJECTION_NATS", true),
            runtime_state_uri: env::var("REHYDRATION_RUNTIME_STATE_URI")
                .unwrap_or_else(|_| "redis://localhost:6379".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ProjectionRuntimeConfig;

    #[test]
    fn defaults_match_generic_projection_runtime_contract() {
        let config = ProjectionRuntimeConfig::from_env();

        assert_eq!(config.nats_url, "nats://nats:4222");
        assert!(config.enabled);
        assert_eq!(config.runtime_state_uri, "redis://localhost:6379");
    }
}
