use rehydration_config::AppConfig;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpAdminServer {
    bind_addr: String,
}

impl HttpAdminServer {
    pub fn new(config: AppConfig) -> Self {
        Self {
            bind_addr: config.admin_bind,
        }
    }

    pub fn describe(&self) -> String {
        format!("http admin placeholder on {}", self.bind_addr)
    }
}

#[cfg(test)]
mod tests {
    use rehydration_config::AppConfig;

    use super::HttpAdminServer;

    #[test]
    fn describe_mentions_bind_address() {
        let server = HttpAdminServer::new(AppConfig {
            service_name: "rehydration-kernel".to_string(),
            grpc_bind: "127.0.0.1:50054".to_string(),
            admin_bind: "127.0.0.1:8080".to_string(),
            graph_uri: "neo4j://localhost:7687".to_string(),
            detail_uri: "redis://localhost:6379".to_string(),
            snapshot_uri: "redis://localhost:6379".to_string(),
            events_subject_prefix: "rehydration".to_string(),
        });

        assert!(server.describe().contains("127.0.0.1:8080"));
    }
}
