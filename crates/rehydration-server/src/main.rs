mod nats_tls;
mod projection_nats_runtime;

use std::process::ExitCode;
use std::sync::Once;

use rehydration_adapter_nats::NatsProjectionConsumer;
use rehydration_adapter_neo4j::Neo4jProjectionReader;
use rehydration_adapter_valkey::{ValkeyContextEventStore, ValkeyNodeDetailStore, ValkeySnapshotStore};
use rehydration_config::{AppConfig, ProjectionRuntimeConfig};
use rehydration_observability::init_observability;
use rehydration_transport_grpc::GrpcServer;

use crate::projection_nats_runtime::connect_projection_runtime;

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!(%error, "rehydration-server failed");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    install_rustls_crypto_provider();
    let config = AppConfig::try_from_env()?;
    let projection_runtime_config = ProjectionRuntimeConfig::try_from_env()?;
    init_observability(&config.service_name);

    let graph_reader = Neo4jProjectionReader::new(config.graph_uri.clone())?;
    let detail_reader = ValkeyNodeDetailStore::new(config.detail_uri.clone())?;
    let grpc_server = GrpcServer::new(
        config.clone(),
        graph_reader.clone(),
        detail_reader.clone(),
        ValkeySnapshotStore::new(config.snapshot_uri.clone())?,
        ValkeyContextEventStore::new(config.snapshot_uri.clone())?,
    );
    let events_consumer = NatsProjectionConsumer::new(config.events_subject_prefix.clone());
    let projection_runtime = connect_projection_runtime(
        &projection_runtime_config,
        &config.events_subject_prefix,
        graph_reader,
        detail_reader,
    )
    .await?;

    tracing::info!(grpc = %grpc_server.describe(), "server ready");
    tracing::info!(events = %events_consumer.describe(), "events ready");
    tracing::info!(projection = %projection_runtime.describe(), "projection ready");
    tracing::info!(
        role = %grpc_server.bootstrap_request().role,
        "query bootstrap"
    );

    let _warmup_bundle = grpc_server.warmup_bundle().await?;

    tokio::try_join!(async { grpc_server.run().await }, async {
        projection_runtime
            .run()
            .await
            .map_err(|error| Box::new(error) as Box<dyn std::error::Error + Send + Sync>)
    })?;

    Ok(())
}

fn install_rustls_crypto_provider() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let _ = tokio_rustls::rustls::crypto::aws_lc_rs::default_provider().install_default();
    });
}
