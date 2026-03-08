use rehydration_adapter_nats::NatsProjectionConsumer;
use rehydration_adapter_neo4j::Neo4jProjectionReader;
use rehydration_adapter_valkey::{ValkeyNodeDetailStore, ValkeySnapshotStore};
use rehydration_config::AppConfig;
use rehydration_observability::init_observability;
use rehydration_transport_grpc::GrpcServer;
use rehydration_transport_http_admin::HttpAdminServer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = AppConfig::from_env();
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

    println!("{}", grpc_server.describe());
    println!("{}", admin_server.describe());
    println!("{}", events_consumer.describe());
    println!(
        "query bootstrap role={}",
        grpc_server.bootstrap_request().role
    );

    let warmup_bundle = grpc_server.warmup_bundle().await?;
    println!(
        "warmup bundle revision={}",
        warmup_bundle.metadata().revision
    );

    grpc_server.run().await
}
