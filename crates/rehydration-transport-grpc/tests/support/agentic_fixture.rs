use std::error::Error;
use std::future::Future;
use std::time::Duration;

use async_nats::Client;
use rehydration_adapter_neo4j::Neo4jProjectionStore;
use rehydration_adapter_valkey::{ValkeyNodeDetailStore, ValkeySnapshotStore};
use rehydration_proto::v1beta1::{
    BundleRenderFormat, GetContextRequest, Phase,
    context_command_service_client::ContextCommandServiceClient,
    context_query_service_client::ContextQueryServiceClient,
};
use testcontainers::GenericImage;
use tokio::time::sleep;
use tonic::transport::Channel;

use crate::agentic_support::agentic_debug::{debug_log, debug_log_value};
use crate::agentic_support::containers::{
    NEO4J_INTERNAL_PORT, NEO4J_PASSWORD, VALKEY_INTERNAL_PORT, clear_neo4j, start_neo4j_container,
    start_valkey_container,
};
use crate::agentic_support::generic_seed_data::{
    FOCUS_NODE_ID, ROOT_NODE_ID, publish_projection_events,
};
use crate::agentic_support::grpc_runtime::{RunningGrpcServer, stop_server};
use crate::agentic_support::nats_container::{
    NATS_INTERNAL_PORT, connect_with_retry, start_nats_container,
};
use crate::agentic_support::projection_runtime::RunningProjectionRuntime;

pub(crate) struct AgenticFixture {
    _neo4j: testcontainers::ContainerAsync<GenericImage>,
    _valkey: testcontainers::ContainerAsync<GenericImage>,
    _nats: testcontainers::ContainerAsync<GenericImage>,
    projection_runtime: RunningProjectionRuntime,
    server: RunningGrpcServer,
    nats_url: String,
    query_client: ContextQueryServiceClient<Channel>,
    #[allow(dead_code)]
    command_client: ContextCommandServiceClient<Channel>,
}

impl AgenticFixture {
    #[allow(dead_code)]
    pub(crate) async fn start() -> Result<Self, Box<dyn Error + Send + Sync>> {
        Self::start_with_seed(ROOT_NODE_ID, FOCUS_NODE_ID, |publisher| async move {
            publish_projection_events(&publisher).await
        })
        .await
    }

    pub(crate) async fn start_with_seed<F, Fut>(
        root_node_id: &str,
        focus_node_id: &str,
        seed_projection: F,
    ) -> Result<Self, Box<dyn Error + Send + Sync>>
    where
        F: FnOnce(Client) -> Fut,
        Fut: Future<Output = Result<(), Box<dyn Error + Send + Sync>>>,
    {
        Self::start_with_seed_and_readiness(root_node_id, focus_node_id, true, seed_projection)
            .await
    }

    pub(crate) async fn start_with_seed_and_readiness<F, Fut>(
        root_node_id: &str,
        focus_node_id: &str,
        require_node_detail: bool,
        seed_projection: F,
    ) -> Result<Self, Box<dyn Error + Send + Sync>>
    where
        F: FnOnce(Client) -> Fut,
        Fut: Future<Output = Result<(), Box<dyn Error + Send + Sync>>>,
    {
        debug_log("starting agentic fixture");
        let neo4j = start_neo4j_container().await?;
        let valkey = start_valkey_container().await?;
        let nats = start_nats_container().await?;

        let neo4j_host = neo4j.get_host().await?;
        let neo4j_port = neo4j.get_host_port_ipv4(NEO4J_INTERNAL_PORT).await?;
        let valkey_host = valkey.get_host().await?;
        let valkey_port = valkey.get_host_port_ipv4(VALKEY_INTERNAL_PORT).await?;
        let nats_port = nats.get_host_port_ipv4(NATS_INTERNAL_PORT).await?;
        debug_log_value("neo4j host", format!("{neo4j_host}:{neo4j_port}"));
        debug_log_value("valkey host", format!("{valkey_host}:{valkey_port}"));
        debug_log_value("nats host", format!("127.0.0.1:{nats_port}"));

        clear_neo4j(format!("neo4j://{neo4j_host}:{neo4j_port}")).await?;
        debug_log("neo4j cleared");

        let graph_store = Neo4jProjectionStore::new(format!(
            "neo4j://neo4j:{NEO4J_PASSWORD}@{neo4j_host}:{neo4j_port}"
        ))?;
        let detail_store = ValkeyNodeDetailStore::new(format!(
            "redis://{valkey_host}:{valkey_port}?key_prefix=rehydration:detail&ttl_seconds=120"
        ))?;
        let snapshot_store = ValkeySnapshotStore::new(format!(
            "redis://{valkey_host}:{valkey_port}?key_prefix=rehydration:snapshot&ttl_seconds=120"
        ))?;

        let nats_url = format!("nats://127.0.0.1:{nats_port}");
        let projection_runtime = RunningProjectionRuntime::start(
            &nats_url,
            "rehydration",
            graph_store.clone(),
            detail_store.clone(),
        )
        .await?;
        debug_log("projection runtime started");

        let server = RunningGrpcServer::start(
            graph_store.clone(),
            detail_store.clone(),
            snapshot_store.clone(),
        )
        .await?;
        debug_log("grpc server started");
        let channel = server.connect_channel().await?;
        debug_log("grpc channel connected");
        let query_client = ContextQueryServiceClient::new(channel.clone());
        let command_client = ContextCommandServiceClient::new(channel);

        let publisher = connect_with_retry(&nats_url).await?;
        seed_projection(publisher.clone()).await?;
        debug_log("projection seed events published");
        wait_for_context_ready(
            query_client.clone(),
            root_node_id,
            focus_node_id,
            require_node_detail,
        )
        .await?;
        debug_log("context became ready");

        Ok(Self {
            _neo4j: neo4j,
            _valkey: valkey,
            _nats: nats,
            projection_runtime,
            server,
            nats_url,
            query_client,
            command_client,
        })
    }

    pub(crate) fn query_client(&self) -> ContextQueryServiceClient<Channel> {
        self.query_client.clone()
    }

    #[allow(dead_code)]
    pub(crate) fn command_client(&self) -> ContextCommandServiceClient<Channel> {
        self.command_client.clone()
    }

    pub(crate) fn nats_url(&self) -> &str {
        &self.nats_url
    }

    pub(crate) async fn shutdown(self) -> Result<(), Box<dyn Error + Send + Sync>> {
        debug_log("shutting down agentic fixture");
        let projection_result = self.projection_runtime.shutdown().await;
        let server_result = stop_server(self.server).await;

        projection_result?;
        server_result
    }
}

async fn wait_for_context_ready(
    mut query_client: ContextQueryServiceClient<Channel>,
    root_node_id: &str,
    focus_node_id: &str,
    require_node_detail: bool,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut last_error: Option<Box<dyn Error + Send + Sync>> = None;

    for _ in 0..40 {
        match query_client
            .get_context(GetContextRequest {
                root_node_id: root_node_id.to_string(),
                role: "implementer".to_string(),
                phase: Phase::Build as i32,
                work_item_id: focus_node_id.to_string(),
                token_budget: 1200,
                requested_scopes: vec!["implementation".to_string()],
                render_format: BundleRenderFormat::Structured as i32,
                include_debug_sections: false,
                depth: 0,
                max_tier: 0,
                rehydration_mode: 0,
            })
            .await
        {
            Ok(response) => {
                let response = response.into_inner();
                if let Some(bundle) = response.bundle
                    && bundle.root_node_id == root_node_id
                    && bundle.bundles.first().is_some_and(|role_bundle| {
                        !role_bundle.neighbor_nodes.is_empty()
                            && (!require_node_detail || !role_bundle.node_details.is_empty())
                    })
                {
                    debug_log("context readiness probe succeeded");
                    return Ok(());
                }
            }
            Err(error) => {
                debug_log_value("context readiness probe error", &error);
                last_error = Some(Box::new(error));
            }
        }

        sleep(Duration::from_millis(200)).await;
    }

    Err(last_error.unwrap_or_else(|| {
        "context projection did not become ready before timeout"
            .to_string()
            .into()
    }))
}
