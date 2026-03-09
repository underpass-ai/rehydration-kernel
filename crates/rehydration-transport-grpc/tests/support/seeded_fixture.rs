use std::error::Error;

use rehydration_adapter_neo4j::Neo4jProjectionReader;
use rehydration_adapter_valkey::{ValkeyNodeDetailStore, ValkeySnapshotStore};
use rehydration_proto::fleet_context_v1::context_service_client::ContextServiceClient;
use testcontainers::GenericImage;
use tonic::transport::Channel;

use crate::support::containers::{
    NEO4J_INTERNAL_PORT, NEO4J_PASSWORD, VALKEY_INTERNAL_PORT, clear_neo4j, start_neo4j_container,
    start_valkey_container,
};
use crate::support::grpc_runtime::{RunningGrpcServer, stop_server};
use crate::support::seed_data::{seed_node_details, seed_projection_graph};

pub(crate) struct SeededCompatibilityFixture {
    _neo4j: testcontainers::ContainerAsync<GenericImage>,
    _valkey: testcontainers::ContainerAsync<GenericImage>,
    server: RunningGrpcServer,
    client: ContextServiceClient<Channel>,
}

impl SeededCompatibilityFixture {
    pub(crate) async fn start() -> Result<Self, Box<dyn Error + Send + Sync>> {
        let neo4j = start_neo4j_container().await?;
        let valkey = start_valkey_container().await?;

        let neo4j_host = neo4j.get_host().await?;
        let neo4j_port = neo4j.get_host_port_ipv4(NEO4J_INTERNAL_PORT).await?;
        let valkey_host = valkey.get_host().await?;
        let valkey_port = valkey.get_host_port_ipv4(VALKEY_INTERNAL_PORT).await?;

        clear_neo4j(format!("neo4j://{neo4j_host}:{neo4j_port}")).await?;

        let graph_store = Neo4jProjectionReader::new(format!(
            "neo4j://neo4j:{NEO4J_PASSWORD}@{neo4j_host}:{neo4j_port}"
        ))?;
        let detail_store = ValkeyNodeDetailStore::new(format!(
            "redis://{valkey_host}:{valkey_port}?key_prefix=rehydration:detail&ttl_seconds=120"
        ))?;
        let snapshot_store = ValkeySnapshotStore::new(format!(
            "redis://{valkey_host}:{valkey_port}?key_prefix=rehydration:snapshot&ttl_seconds=120"
        ))?;

        seed_projection_graph(&graph_store).await?;
        seed_node_details(&detail_store).await?;

        let server = RunningGrpcServer::start(
            graph_store.clone(),
            detail_store.clone(),
            snapshot_store.clone(),
        )
        .await?;
        let client = ContextServiceClient::new(server.connect_channel().await?);

        Ok(Self {
            _neo4j: neo4j,
            _valkey: valkey,
            server,
            client,
        })
    }

    pub(crate) fn client(&mut self) -> &mut ContextServiceClient<Channel> {
        &mut self.client
    }

    pub(crate) async fn shutdown(self) -> Result<(), Box<dyn Error + Send + Sync>> {
        stop_server(self.server).await
    }
}
