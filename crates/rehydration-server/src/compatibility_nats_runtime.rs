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
