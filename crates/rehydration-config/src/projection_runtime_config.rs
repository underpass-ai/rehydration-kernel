use std::env;
use std::io;

use crate::NatsTlsConfig;

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
        Ok(Self {
            nats_url: lookup("NATS_URL").unwrap_or_else(|| "nats://nats:4222".to_string()),
            enabled: lookup("ENABLE_PROJECTION_NATS")
                .map(|value| crate::env_bool::parse_bool_value(&value))
                .unwrap_or(true),
            runtime_state_uri: lookup("REHYDRATION_RUNTIME_STATE_URI")
                .unwrap_or_else(|| "redis://localhost:6379".to_string()),
            nats_tls: NatsTlsConfig::from_lookup(&lookup)?,
        })
    }

    pub fn disabled() -> Self {
        Self {
            nats_url: "nats://nats:4222".to_string(),
            enabled: false,
            runtime_state_uri: "redis://localhost:6379".to_string(),
            nats_tls: NatsTlsConfig::disabled(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::ProjectionRuntimeConfig;

    #[test]
    fn defaults_match_generic_projection_runtime_contract() {
        let config = ProjectionRuntimeConfig::from_env();

        assert_eq!(config.nats_url, "nats://nats:4222");
        assert!(config.enabled);
        assert_eq!(config.runtime_state_uri, "redis://localhost:6379");
        assert_eq!(config.nats_tls, crate::NatsTlsConfig::disabled());
    }

    #[test]
    fn projection_runtime_loads_mutual_nats_tls() {
        let env = [
            ("NATS_TLS_MODE", "mutual"),
            ("NATS_TLS_CA_PATH", "/tmp/ca.pem"),
            ("NATS_TLS_CERT_PATH", "/tmp/client.pem"),
            ("NATS_TLS_KEY_PATH", "/tmp/client.key"),
            ("NATS_TLS_FIRST", "true"),
        ]
        .into_iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect::<BTreeMap<_, _>>();

        let config = ProjectionRuntimeConfig::from_lookup(|key| env.get(key).cloned())
            .expect("mutual TLS config should load");

        assert_eq!(config.nats_tls.mode, crate::NatsTlsMode::Mutual);
        assert_eq!(
            config.nats_tls.cert_path.as_deref(),
            Some(std::path::Path::new("/tmp/client.pem"))
        );
        assert_eq!(
            config.nats_tls.key_path.as_deref(),
            Some(std::path::Path::new("/tmp/client.key"))
        );
        assert!(config.nats_tls.tls_first);
    }

    #[test]
    fn projection_runtime_rejects_tls_first_without_tls_mode() {
        let env = [("NATS_TLS_FIRST", "true")]
            .into_iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect::<BTreeMap<_, _>>();

        let error = ProjectionRuntimeConfig::from_lookup(|key| env.get(key).cloned())
            .expect_err("tls_first should require TLS mode");

        assert!(error.to_string().contains("NATS_TLS_FIRST"));
    }
}
