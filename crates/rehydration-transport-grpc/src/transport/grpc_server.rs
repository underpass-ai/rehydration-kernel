use std::fs;
use std::future::{Future, pending};
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;

use rehydration_application::{
    AdminCommandApplicationService, AdminQueryApplicationService, ApplicationError,
    CommandApplicationService, DEFAULT_NATIVE_GRAPH_TRAVERSAL_DEPTH, QueryApplicationService,
    RehydrationApplication, UpdateContextUseCase,
};
use rehydration_config::{AppConfig, GrpcTlsConfig, GrpcTlsMode};
use rehydration_domain::{
    GraphNeighborhoodReader, NodeDetailReader, RehydrationBundle, SnapshotStore,
};
use rehydration_proto::fleet_context_v1::context_service_server::ContextServiceServer;
use rehydration_proto::v1alpha1::{
    BundleRenderFormat, FILE_DESCRIPTOR_SET, GetContextRequest, Phase,
    context_admin_service_server::ContextAdminServiceServer,
    context_command_service_server::ContextCommandServiceServer,
    context_query_service_server::ContextQueryServiceServer,
};
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::{Certificate, Identity, Server, ServerTlsConfig};

use crate::transport::{
    AdminGrpcService, CommandGrpcService, ContextCompatibilityGrpcService, QueryGrpcService,
};

#[derive(Debug)]
pub struct GrpcServer<G, D, S> {
    bind_addr: String,
    grpc_tls: GrpcTlsConfig,
    query_application: Arc<QueryApplicationService<G, D, S>>,
    admin_query_application: Arc<AdminQueryApplicationService<G, D>>,
    admin_command_application: Arc<AdminCommandApplicationService>,
    command_application: Arc<CommandApplicationService>,
    capability_name: &'static str,
}

impl<G, D, S> GrpcServer<G, D, S>
where
    G: GraphNeighborhoodReader + Send + Sync + 'static,
    D: NodeDetailReader + Send + Sync + 'static,
    S: SnapshotStore + Send + Sync + 'static,
{
    pub fn new(config: AppConfig, graph_reader: G, detail_reader: D, snapshot_store: S) -> Self {
        let AppConfig {
            grpc_bind,
            grpc_tls,
            ..
        } = config;
        let graph_reader = Arc::new(graph_reader);
        let detail_reader = Arc::new(detail_reader);
        let snapshot_store = Arc::new(snapshot_store);
        let generator_version = env!("CARGO_PKG_VERSION");
        let update_context = Arc::new(UpdateContextUseCase::new(generator_version));

        Self {
            bind_addr: grpc_bind,
            grpc_tls,
            query_application: Arc::new(QueryApplicationService::new(
                Arc::clone(&graph_reader),
                Arc::clone(&detail_reader),
                Arc::clone(&snapshot_store),
                generator_version,
            )),
            admin_query_application: Arc::new(AdminQueryApplicationService::new(
                Arc::clone(&graph_reader),
                Arc::clone(&detail_reader),
                generator_version,
            )),
            admin_command_application: Arc::new(AdminCommandApplicationService),
            command_application: Arc::new(CommandApplicationService::new(update_context)),
            capability_name: RehydrationApplication::capability_name(),
        }
    }

    pub fn describe(&self) -> String {
        format!(
            "grpc transport for {} on {} (tls={})",
            self.capability_name,
            self.bind_addr,
            self.grpc_tls.mode.as_str()
        )
    }

    pub fn bootstrap_request(&self) -> GetContextRequest {
        GetContextRequest {
            root_node_id: "bootstrap-node".to_string(),
            role: "system".to_string(),
            phase: Phase::Build as i32,
            work_item_id: String::new(),
            token_budget: 4096,
            requested_scopes: Vec::new(),
            render_format: BundleRenderFormat::Structured as i32,
            include_debug_sections: false,
            depth: DEFAULT_NATIVE_GRAPH_TRAVERSAL_DEPTH,
        }
    }

    pub fn descriptor_set(&self) -> &'static [u8] {
        FILE_DESCRIPTOR_SET
    }

    pub fn query_service(&self) -> QueryGrpcService<G, D, S> {
        QueryGrpcService::new(Arc::clone(&self.query_application))
    }

    pub fn command_service(&self) -> CommandGrpcService {
        CommandGrpcService::new(Arc::clone(&self.command_application))
    }

    pub fn query_application(&self) -> Arc<QueryApplicationService<G, D, S>> {
        Arc::clone(&self.query_application)
    }

    pub fn command_application(&self) -> Arc<CommandApplicationService> {
        Arc::clone(&self.command_application)
    }

    pub fn admin_service(&self) -> AdminGrpcService<G, D> {
        AdminGrpcService::new(
            Arc::clone(&self.admin_query_application),
            Arc::clone(&self.admin_command_application),
        )
    }

    pub fn compatibility_service(&self) -> ContextCompatibilityGrpcService<G, D, S> {
        ContextCompatibilityGrpcService::new(
            Arc::clone(&self.query_application),
            Arc::clone(&self.admin_query_application),
            Arc::clone(&self.command_application),
        )
    }

    pub async fn warmup_bundle(&self) -> Result<RehydrationBundle, ApplicationError> {
        self.query_application.warmup_bundle().await
    }

    pub async fn run(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let bind_addr: SocketAddr = self.bind_addr.parse()?;
        let listener = TcpListener::bind(bind_addr).await?;

        self.serve_with_listener_shutdown(listener, pending()).await
    }

    pub async fn serve_with_listener_shutdown<F>(
        self,
        listener: TcpListener,
        shutdown: F,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.transport_builder()?
            .add_service(ContextServiceServer::new(self.compatibility_service()))
            .add_service(ContextQueryServiceServer::new(self.query_service()))
            .add_service(ContextCommandServiceServer::new(self.command_service()))
            .add_service(ContextAdminServiceServer::new(self.admin_service()))
            .serve_with_incoming_shutdown(TcpListenerStream::new(listener), shutdown)
            .await?;

        Ok(())
    }

    fn transport_builder(&self) -> Result<Server, Box<dyn std::error::Error + Send + Sync>> {
        let builder = Server::builder();

        match self.grpc_tls.mode {
            GrpcTlsMode::Disabled => Ok(builder),
            GrpcTlsMode::Server | GrpcTlsMode::Mutual => builder
                .tls_config(load_server_tls_config(&self.grpc_tls)?)
                .map_err(|error| Box::new(error) as Box<dyn std::error::Error + Send + Sync>),
        }
    }
}

fn load_server_tls_config(
    grpc_tls: &GrpcTlsConfig,
) -> Result<ServerTlsConfig, Box<dyn std::error::Error + Send + Sync>> {
    let cert_pem = read_required_pem(
        grpc_tls.cert_path.as_deref(),
        "REHYDRATION_GRPC_TLS_CERT_PATH",
    )?;
    let key_pem = read_required_pem(
        grpc_tls.key_path.as_deref(),
        "REHYDRATION_GRPC_TLS_KEY_PATH",
    )?;
    let identity = Identity::from_pem(cert_pem, key_pem);
    let mut tls_config = ServerTlsConfig::new().identity(identity);

    if grpc_tls.mode == GrpcTlsMode::Mutual {
        let client_ca_pem = read_required_pem(
            grpc_tls.client_ca_path.as_deref(),
            "REHYDRATION_GRPC_TLS_CLIENT_CA_PATH",
        )?;
        tls_config = tls_config.client_ca_root(Certificate::from_pem(client_ca_pem));
    }

    Ok(tls_config)
}

fn read_required_pem(
    path: Option<&Path>,
    env_name: &str,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    let path = path.ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("{env_name} must be configured when gRPC TLS is enabled"),
        )
    })?;

    fs::read(path).map_err(|error| {
        Box::new(std::io::Error::new(
            error.kind(),
            format!(
                "failed to read {} from {}: {error}",
                env_name,
                path.display()
            ),
        )) as Box<dyn std::error::Error + Send + Sync>
    })
}
