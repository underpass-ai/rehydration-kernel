use std::env;
use std::io;

use crate::GrpcTlsConfig;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    pub service_name: String,
    pub grpc_bind: String,
    pub grpc_tls: GrpcTlsConfig,
    pub graph_uri: String,
    pub detail_uri: String,
    pub snapshot_uri: String,
    pub events_subject_prefix: String,
}

impl AppConfig {
    pub fn from_env() -> Self {
        Self::try_from_env().expect("REHYDRATION_* application config should be valid")
    }

    pub fn try_from_env() -> io::Result<Self> {
        Self::from_lookup(|key| env::var(key).ok())
    }

    fn from_lookup<F>(lookup: F) -> io::Result<Self>
    where
        F: Fn(&str) -> Option<String>,
    {
        Ok(Self {
            service_name: lookup_or_default(
                &lookup,
                "REHYDRATION_SERVICE_NAME",
                "rehydration-kernel",
            ),
            grpc_bind: lookup_or_default(&lookup, "REHYDRATION_GRPC_BIND", "0.0.0.0:50054"),
            grpc_tls: GrpcTlsConfig::from_lookup(&lookup)?,
            graph_uri: lookup_or_default(
                &lookup,
                "REHYDRATION_GRAPH_URI",
                "neo4j://localhost:7687",
            ),
            detail_uri: lookup_or_default(
                &lookup,
                "REHYDRATION_DETAIL_URI",
                "redis://localhost:6379",
            ),
            snapshot_uri: lookup_or_default(
                &lookup,
                "REHYDRATION_SNAPSHOT_URI",
                "redis://localhost:6379",
            ),
            events_subject_prefix: lookup_or_default(
                &lookup,
                "REHYDRATION_EVENTS_PREFIX",
                "rehydration",
            ),
        })
    }
}

fn lookup_or_default<F>(lookup: &F, key: &str, default_value: &str) -> String
where
    F: Fn(&str) -> Option<String>,
{
    lookup(key).unwrap_or_else(|| default_value.to_string())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::{GrpcTlsConfig, GrpcTlsMode};

    use super::AppConfig;

    #[test]
    fn default_service_name_is_stable() {
        let config = AppConfig::from_lookup(|_| None).expect("defaults should load");

        assert_eq!(config.service_name, "rehydration-kernel");
        assert_eq!(config.grpc_bind, "0.0.0.0:50054");
        assert_eq!(config.grpc_tls, GrpcTlsConfig::disabled());
        assert_eq!(config.graph_uri, "neo4j://localhost:7687");
        assert_eq!(config.detail_uri, "redis://localhost:6379");
        assert_eq!(config.snapshot_uri, "redis://localhost:6379");
        assert_eq!(config.events_subject_prefix, "rehydration");
    }

    #[test]
    fn grpc_tls_server_mode_reads_paths() {
        let env = [
            ("REHYDRATION_GRPC_TLS_MODE", "server"),
            ("REHYDRATION_GRPC_TLS_CERT_PATH", "/tmp/server.crt"),
            ("REHYDRATION_GRPC_TLS_KEY_PATH", "/tmp/server.key"),
        ]
        .into_iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect::<BTreeMap<_, _>>();

        let config = AppConfig::from_lookup(|key| env.get(key).cloned())
            .expect("server TLS config should load");

        assert_eq!(config.grpc_tls.mode, GrpcTlsMode::Server);
        assert_eq!(
            config.grpc_tls.cert_path.as_deref(),
            Some(std::path::Path::new("/tmp/server.crt"))
        );
        assert_eq!(
            config.grpc_tls.key_path.as_deref(),
            Some(std::path::Path::new("/tmp/server.key"))
        );
        assert_eq!(config.grpc_tls.client_ca_path, None);
    }

    #[test]
    fn grpc_tls_mutual_mode_requires_client_ca() {
        let env = [
            ("REHYDRATION_GRPC_TLS_MODE", "mutual"),
            ("REHYDRATION_GRPC_TLS_CERT_PATH", "/tmp/server.crt"),
            ("REHYDRATION_GRPC_TLS_KEY_PATH", "/tmp/server.key"),
        ]
        .into_iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect::<BTreeMap<_, _>>();

        let error = AppConfig::from_lookup(|key| env.get(key).cloned())
            .expect_err("mutual TLS should require a client CA");

        assert!(
            error
                .to_string()
                .contains("REHYDRATION_GRPC_TLS_CLIENT_CA_PATH"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn grpc_tls_mode_rejects_invalid_value() {
        let env = [("REHYDRATION_GRPC_TLS_MODE", "banana")]
            .into_iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect::<BTreeMap<_, _>>();

        let error = AppConfig::from_lookup(|key| env.get(key).cloned())
            .expect_err("invalid mode should fail");

        assert!(
            error
                .to_string()
                .contains("unsupported REHYDRATION_GRPC_TLS_MODE")
        );
    }
}
