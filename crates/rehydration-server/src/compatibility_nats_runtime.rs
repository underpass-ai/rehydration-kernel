use std::error::Error;

use rehydration_adapter_nats::{ContextAsyncApplication, NatsCompatibilityRuntime};
use rehydration_config::CompatibilityNatsConfig;
use rehydration_domain::{GraphNeighborhoodReader, NodeDetailReader, SnapshotStore};
use rehydration_transport_grpc::GrpcServer;

use crate::nats_tls::adapter_nats_tls_config;

pub enum CompatibilityRuntime<S> {
    Enabled(Box<NatsCompatibilityRuntime<S>>),
    Disabled,
}

impl<S> CompatibilityRuntime<S>
where
    S: rehydration_adapter_nats::ContextAsyncService + Send + Sync + 'static,
{
    pub fn describe(&self) -> String {
        match self {
            Self::Enabled(runtime) => runtime.describe(),
            Self::Disabled => "nats compatibility runtime disabled".to_string(),
        }
    }

    pub async fn run(self) -> Result<(), Box<dyn Error + Send + Sync>> {
        match self {
            Self::Enabled(runtime) => runtime
                .run()
                .await
                .map_err(|error| Box::new(error) as Box<dyn Error + Send + Sync>),
            Self::Disabled => Ok(()),
        }
    }
}

pub async fn connect_compatibility_runtime<G, D, S>(
    grpc_server: &GrpcServer<G, D, S>,
    config: &CompatibilityNatsConfig,
) -> Result<CompatibilityRuntime<ContextAsyncApplication<G, D, S>>, Box<dyn Error + Send + Sync>>
where
    G: GraphNeighborhoodReader + Send + Sync + 'static,
    D: NodeDetailReader + Send + Sync + 'static,
    S: SnapshotStore + Send + Sync + 'static,
{
    if !config.enabled {
        return Ok(CompatibilityRuntime::Disabled);
    }

    NatsCompatibilityRuntime::connect(
        &config.url,
        &adapter_nats_tls_config(&config.tls),
        ContextAsyncApplication::new(
            grpc_server.command_application(),
            grpc_server.query_application(),
        ),
    )
    .await
    .map(Box::new)
    .map(CompatibilityRuntime::Enabled)
    .map_err(|error| Box::new(error) as Box<dyn Error + Send + Sync>)
}

#[cfg(test)]
mod tests {
    use rehydration_config::{AppConfig, GrpcTlsConfig, NatsTlsConfig};
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
    async fn disabled_compatibility_nats_returns_disabled_runtime() {
        let grpc_server = GrpcServer::new(
            AppConfig {
                service_name: "rehydration-kernel".to_string(),
                grpc_bind: "127.0.0.1:50054".to_string(),
                admin_bind: "127.0.0.1:8080".to_string(),
                grpc_tls: GrpcTlsConfig::disabled(),
                graph_uri: "neo4j://localhost:7687".to_string(),
                detail_uri: "redis://localhost:6379".to_string(),
                snapshot_uri: "redis://localhost:6379".to_string(),
                events_subject_prefix: "rehydration".to_string(),
            },
            EmptyGraphNeighborhoodReader,
            EmptyNodeDetailReader,
            NoopSnapshotStore,
        );

        let runtime = connect_compatibility_runtime(
            &grpc_server,
            &rehydration_config::CompatibilityNatsConfig {
                url: "nats://127.0.0.1:4222".to_string(),
                enabled: false,
                tls: NatsTlsConfig::disabled(),
            },
        )
        .await
        .expect("disabled nats should still allow the kernel runtime to start");

        assert_eq!(runtime.describe(), "nats compatibility runtime disabled");
    }

    #[tokio::test]
    async fn disabled_runtime_run_returns_ok() {
        let runtime = super::CompatibilityRuntime::<
            rehydration_adapter_nats::ContextAsyncApplication<
                EmptyGraphNeighborhoodReader,
                EmptyNodeDetailReader,
                NoopSnapshotStore,
            >,
        >::Disabled;

        runtime
            .run()
            .await
            .expect("disabled runtime should not fail when run");
    }

    #[tokio::test]
    async fn enabled_runtime_surfaces_connection_errors() {
        let grpc_server = GrpcServer::new(
            AppConfig {
                service_name: "rehydration-kernel".to_string(),
                grpc_bind: "127.0.0.1:50054".to_string(),
                admin_bind: "127.0.0.1:8080".to_string(),
                grpc_tls: GrpcTlsConfig::disabled(),
                graph_uri: "neo4j://localhost:7687".to_string(),
                detail_uri: "redis://localhost:6379".to_string(),
                snapshot_uri: "redis://localhost:6379".to_string(),
                events_subject_prefix: "rehydration".to_string(),
            },
            EmptyGraphNeighborhoodReader,
            EmptyNodeDetailReader,
            NoopSnapshotStore,
        );

        let error = match connect_compatibility_runtime(
            &grpc_server,
            &rehydration_config::CompatibilityNatsConfig {
                url: "nats://127.0.0.1:1".to_string(),
                enabled: true,
                tls: NatsTlsConfig::disabled(),
            },
        )
        .await
        {
            Ok(_) => panic!("invalid nats endpoint should fail when runtime is enabled"),
            Err(error) => error,
        };

        assert!(
            error.to_string().contains("connection"),
            "unexpected error: {error}"
        );
    }
}
