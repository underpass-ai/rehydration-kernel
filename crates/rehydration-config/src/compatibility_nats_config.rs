use std::env;
use std::io;

use crate::{NatsTlsConfig, nats_tls_config::NatsEndpointConfig};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompatibilityNatsConfig {
    pub url: String,
    pub enabled: bool,
    pub tls: NatsTlsConfig,
}

impl CompatibilityNatsConfig {
    pub fn from_env() -> Self {
        Self::try_from_env().expect("NATS_* compatibility config should be valid")
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
            url: endpoint.url,
            enabled: lookup("ENABLE_NATS")
                .map(|value| crate::env_bool::parse_bool_value(&value))
                .unwrap_or(true),
            tls: endpoint.tls,
        })
    }

    pub fn disabled() -> Self {
        let endpoint = NatsEndpointConfig::disabled();

        Self {
            url: endpoint.url,
            enabled: false,
            tls: endpoint.tls,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CompatibilityNatsConfig;

    #[test]
    fn defaults_match_external_compatibility_contract() {
        let config = CompatibilityNatsConfig::from_env();

        assert_eq!(config.url, "nats://nats:4222");
        assert!(config.enabled);
        assert_eq!(config.tls, crate::NatsTlsConfig::disabled());
    }
}
