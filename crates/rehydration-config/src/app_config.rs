use std::env;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    pub service_name: String,
    pub grpc_bind: String,
    pub admin_bind: String,
    pub graph_uri: String,
    pub detail_uri: String,
    pub snapshot_uri: String,
    pub events_subject_prefix: String,
}

impl AppConfig {
    pub fn from_env() -> Self {
        Self {
            service_name: env::var("REHYDRATION_SERVICE_NAME")
                .unwrap_or_else(|_| "rehydration-kernel".to_string()),
            grpc_bind: env::var("REHYDRATION_GRPC_BIND")
                .unwrap_or_else(|_| "0.0.0.0:50054".to_string()),
            admin_bind: env::var("REHYDRATION_ADMIN_BIND")
                .unwrap_or_else(|_| "0.0.0.0:8080".to_string()),
            graph_uri: env::var("REHYDRATION_GRAPH_URI")
                .unwrap_or_else(|_| "neo4j://localhost:7687".to_string()),
            detail_uri: env::var("REHYDRATION_DETAIL_URI")
                .unwrap_or_else(|_| "redis://localhost:6379".to_string()),
            snapshot_uri: env::var("REHYDRATION_SNAPSHOT_URI")
                .unwrap_or_else(|_| "redis://localhost:6379".to_string()),
            events_subject_prefix: env::var("REHYDRATION_EVENTS_PREFIX")
                .unwrap_or_else(|_| "rehydration".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AppConfig;

    #[test]
    fn default_service_name_is_stable() {
        let config = AppConfig::from_env();
        assert_eq!(config.service_name, "rehydration-kernel");
    }
}
