use rehydration_adapter_nats::NatsProjectionConsumer;
use rehydration_adapter_neo4j::Neo4jProjectionReader;
use rehydration_adapter_valkey::ValkeySnapshotStore;
use rehydration_config::AppConfig;
use rehydration_observability::init_observability;
use rehydration_transport_grpc::GrpcServer;
use rehydration_transport_http_admin::HttpAdminServer;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = AppConfig::from_env();
    init_observability(&config.service_name);

    let grpc_server = GrpcServer::new(
        config.clone(),
        Neo4jProjectionReader::new(config.graph_uri.clone()),
        ValkeySnapshotStore::new(config.snapshot_uri.clone()),
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

    let warmup_bundle = grpc_server.warmup_bundle()?;
    println!(
        "warmup bundle revision={}",
        warmup_bundle.metadata().revision
    );

    Ok(())
}
