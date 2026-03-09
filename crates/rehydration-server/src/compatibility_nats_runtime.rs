use std::error::Error;

use rehydration_adapter_nats::{ContextAsyncApplication, NatsCompatibilityRuntime};
use rehydration_config::CompatibilityNatsConfig;
use rehydration_domain::{GraphNeighborhoodReader, NodeDetailReader, SnapshotStore};
use rehydration_transport_grpc::GrpcServer;

pub async fn connect_compatibility_runtime<G, D, S>(
    grpc_server: &GrpcServer<G, D, S>,
    config: &CompatibilityNatsConfig,
) -> Result<NatsCompatibilityRuntime<ContextAsyncApplication<G, D, S>>, Box<dyn Error + Send + Sync>>
where
    G: GraphNeighborhoodReader + Send + Sync + 'static,
    D: NodeDetailReader + Send + Sync + 'static,
    S: SnapshotStore + Send + Sync + 'static,
{
    if !config.enabled {
        return Err("NATS is required for Context Service to function. Set ENABLE_NATS=true or remove the environment variable (defaults to true).".into());
    }

    NatsCompatibilityRuntime::connect(
        &config.url,
        ContextAsyncApplication::new(
            grpc_server.command_application(),
            grpc_server.query_application(),
        ),
    )
    .await
    .map_err(|error| Box::new(error) as Box<dyn Error + Send + Sync>)
}

#[cfg(test)]
mod tests {
    use rehydration_config::AppConfig;
    use rehydration_domain::{
        NodeDetailProjection, NodeNeighborhood, PortError, SnapshotSaveOptions,
    };
    use rehydration_transport_grpc::GrpcServer;

    use super::connect_compatibility_runtime;

    #[derive(Debug)]
    struct EmptyGraphNeighborhoodReader;

    impl rehydration_domain::GraphNeighborhoodReader for EmptyGraphNeighborhoodReader {
        async fn load_neighborhood(
            &self,
            _root_node_id: &str,
        ) -> Result<Option<NodeNeighborhood>, PortError> {
            Ok(None)
        }
    }

    #[derive(Debug)]
    struct EmptyNodeDetailReader;

    impl rehydration_domain::NodeDetailReader for EmptyNodeDetailReader {
        async fn load_node_detail(
            &self,
            _node_id: &str,
        ) -> Result<Option<NodeDetailProjection>, PortError> {
            Ok(None)
        }
    }

    #[derive(Debug)]
    struct NoopSnapshotStore;

    impl rehydration_domain::SnapshotStore for NoopSnapshotStore {
        async fn save_bundle_with_options(
            &self,
            _bundle: &rehydration_domain::RehydrationBundle,
            _options: SnapshotSaveOptions,
        ) -> Result<(), PortError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn disabled_compatibility_nats_fails_fast() {
        let grpc_server = GrpcServer::new(
            AppConfig {
                service_name: "rehydration-kernel".to_string(),
                grpc_bind: "127.0.0.1:50054".to_string(),
                admin_bind: "127.0.0.1:8080".to_string(),
                graph_uri: "neo4j://localhost:7687".to_string(),
                detail_uri: "redis://localhost:6379".to_string(),
                snapshot_uri: "redis://localhost:6379".to_string(),
                events_subject_prefix: "rehydration".to_string(),
            },
            EmptyGraphNeighborhoodReader,
            EmptyNodeDetailReader,
            NoopSnapshotStore,
        );

        let error = connect_compatibility_runtime(
            &grpc_server,
            &rehydration_config::CompatibilityNatsConfig {
                url: "nats://127.0.0.1:4222".to_string(),
                enabled: false,
            },
        )
        .await
        .expect_err("disabled nats should fail fast");

        assert!(
            error
                .to_string()
                .contains("NATS is required for Context Service to function")
        );
    }
}
