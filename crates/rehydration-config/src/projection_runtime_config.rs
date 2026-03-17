use std::env;
use std::io;

use crate::{NatsTlsConfig, nats_tls_config::NatsEndpointConfig};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionRuntimeConfig {
    pub nats_url: String,
    pub enabled: bool,
    pub runtime_state_uri: String,
    pub nats_tls: NatsTlsConfig,
}

impl ProjectionRuntimeConfig {
    pub fn from_env() -> Self {
        Self::try_from_env().expect("projection runtime config should be valid")
    }

    pub fn try_from_env() -> io::Result<Self> {
        Self::from_lookup(|key| env::var(key).ok())
    }

    fn from_lookup<F>(lookup: F) -> io::Result<Self>
    where
        F: Fn(&str) -> Option<String>,
    {
        let endpoint = NatsEndpointConfig::from_lookup(&lookup)?;

        Ok(Self {
            nats_url: endpoint.url,
            enabled: lookup("ENABLE_PROJECTION_NATS")
                .map(|value| crate::env_bool::parse_bool_value(&value))
                .unwrap_or(true),
            runtime_state_uri: lookup("REHYDRATION_RUNTIME_STATE_URI")
                .unwrap_or_else(|| "redis://localhost:6379".to_string()),
            nats_tls: endpoint.tls,
        })
    }

    pub fn disabled() -> Self {
        let endpoint = NatsEndpointConfig::disabled();

        Self {
            nats_url: endpoint.url,
            enabled: false,
            runtime_state_uri: "redis://localhost:6379".to_string(),
            nats_tls: endpoint.tls,
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
        assert_eq!(config.nats_tls, crate::NatsTlsConfig::disabled());
    }
}
