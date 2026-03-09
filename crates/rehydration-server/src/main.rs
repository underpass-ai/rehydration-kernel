mod compatibility_nats_runtime;

use rehydration_adapter_nats::NatsProjectionConsumer;
use rehydration_adapter_neo4j::Neo4jProjectionReader;
use rehydration_adapter_valkey::{ValkeyNodeDetailStore, ValkeySnapshotStore};
use rehydration_config::{AppConfig, CompatibilityNatsConfig};
use rehydration_observability::init_observability;
use rehydration_transport_grpc::GrpcServer;
use rehydration_transport_http_admin::HttpAdminServer;

use crate::compatibility_nats_runtime::connect_compatibility_runtime;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = AppConfig::from_env();
    let compatibility_nats_config = CompatibilityNatsConfig::from_env();
    init_observability(&config.service_name);

    let graph_reader = Neo4jProjectionReader::new(config.graph_uri.clone())?;
    let detail_reader = ValkeyNodeDetailStore::new(config.detail_uri.clone())?;
    let grpc_server = GrpcServer::new(
        config.clone(),
        graph_reader,
        detail_reader,
        ValkeySnapshotStore::new(config.snapshot_uri.clone())?,
    );
    let admin_server = HttpAdminServer::new(config.clone());
    let events_consumer = NatsProjectionConsumer::new(config.events_subject_prefix.clone());
    let compatibility_runtime =
        connect_compatibility_runtime(&grpc_server, &compatibility_nats_config).await?;

    println!("{}", grpc_server.describe());
    println!("{}", admin_server.describe());
    println!("{}", events_consumer.describe());
    println!("{}", compatibility_runtime.describe());
    println!(
        "query bootstrap role={}",
        grpc_server.bootstrap_request().role
    );

    let warmup_bundle = grpc_server.warmup_bundle().await?;
    println!(
        "warmup bundle revision={}",
        warmup_bundle.metadata().revision
    );

    tokio::try_join!(async { grpc_server.run().await }, async {
        compatibility_runtime
            .run()
            .await
            .map_err(|error| Box::new(error) as Box<dyn std::error::Error + Send + Sync>)
    })?;

    Ok(())
}
