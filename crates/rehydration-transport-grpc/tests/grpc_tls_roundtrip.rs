use std::fs;
use std::io;
use std::path::PathBuf;

use rehydration_config::{AppConfig, GrpcTlsConfig, GrpcTlsMode};
use rehydration_domain::{
    GraphNeighborhoodReader, NodeDetailProjection, NodeDetailReader, NodeNeighborhood, PortError,
    RehydrationBundle, SnapshotSaveOptions, SnapshotStore,
};
use rehydration_proto::v1alpha1::{
    BundleRenderFormat, GetContextRequest, Phase,
    context_query_service_client::ContextQueryServiceClient,
};
use rehydration_transport_grpc::GrpcServer;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::time::sleep;
use tonic::transport::{Certificate, Channel, ClientTlsConfig, Endpoint, Identity};

const CA_CERT: &str = include_str!("fixtures/tls/ca.crt");
const SERVER_CERT: &str = include_str!("fixtures/tls/server.crt");
const SERVER_KEY: &str = include_str!("fixtures/tls/server-key.pem");
const CLIENT_CERT: &str = include_str!("fixtures/tls/client.crt");
const CLIENT_KEY: &str = include_str!("fixtures/tls/client-key.pem");

struct EmptyGraphNeighborhoodReader;

impl GraphNeighborhoodReader for EmptyGraphNeighborhoodReader {
    async fn load_neighborhood(
        &self,
        _root_node_id: &str,
    ) -> Result<Option<NodeNeighborhood>, PortError> {
        Ok(None)
    }
}

struct EmptyNodeDetailReader;

impl NodeDetailReader for EmptyNodeDetailReader {
    async fn load_node_detail(
        &self,
        _node_id: &str,
    ) -> Result<Option<NodeDetailProjection>, PortError> {
        Ok(None)
    }
}

struct NoopSnapshotStore;

impl SnapshotStore for NoopSnapshotStore {
    async fn save_bundle_with_options(
        &self,
        _bundle: &RehydrationBundle,
        _options: SnapshotSaveOptions,
    ) -> Result<(), PortError> {
        Ok(())
    }
}

struct TlsFixturePaths {
    _dir: TempDir,
    server_cert: PathBuf,
    server_key: PathBuf,
    client_ca: PathBuf,
}

impl TlsFixturePaths {
    fn new() -> io::Result<Self> {
        let dir = tempfile::tempdir()?;
        let server_cert = write_fixture(&dir, "server.crt", SERVER_CERT)?;
        let server_key = write_fixture(&dir, "server.key", SERVER_KEY)?;
        let client_ca = write_fixture(&dir, "ca.crt", CA_CERT)?;

        Ok(Self {
            _dir: dir,
            server_cert,
            server_key,
            client_ca,
        })
    }
}

#[tokio::test]
async fn grpc_server_supports_tls_roundtrip() {
    let tls_fixture = TlsFixturePaths::new().expect("TLS fixture files should be written");
    let running = start_server(GrpcTlsConfig {
        mode: GrpcTlsMode::Server,
        cert_path: Some(tls_fixture.server_cert.clone()),
        key_path: Some(tls_fixture.server_key.clone()),
        client_ca_path: None,
    })
    .await
    .expect("TLS server should start");

    let channel = connect_tls_channel(&running.endpoint, false)
        .await
        .expect("TLS client should connect");
    let mut query_client = ContextQueryServiceClient::new(channel);
    let response = get_context(&mut query_client)
        .await
        .expect("TLS roundtrip should succeed")
        .into_inner();

    assert_eq!(
        response.bundle.expect("bundle should exist").root_node_id,
        "case-123"
    );

    stop_server(running)
        .await
        .expect("server should stop cleanly");
}

#[tokio::test]
async fn grpc_server_rejects_clients_without_certificate_in_mutual_mode() {
    let tls_fixture = TlsFixturePaths::new().expect("TLS fixture files should be written");
    let running = start_server(GrpcTlsConfig {
        mode: GrpcTlsMode::Mutual,
        cert_path: Some(tls_fixture.server_cert.clone()),
        key_path: Some(tls_fixture.server_key.clone()),
        client_ca_path: Some(tls_fixture.client_ca.clone()),
    })
    .await
    .expect("mTLS server should start");

    let connection = connect_tls_channel(&running.endpoint, false).await;
    match connection {
        Ok(channel) => {
            let mut query_client = ContextQueryServiceClient::new(channel);
            let error = get_context(&mut query_client)
                .await
                .expect_err("mTLS request without client cert should fail");
            let message = error.to_string().to_ascii_lowercase();
            assert!(
                message.contains("tls")
                    || message.contains("certificate")
                    || message.contains("handshake")
                    || message.contains("transport error")
                    || message.contains("unknown ca"),
                "unexpected error: {error}"
            );
        }
        Err(error) => {
            let message = error.to_string().to_ascii_lowercase();
            assert!(
                message.contains("tls")
                    || message.contains("certificate")
                    || message.contains("handshake")
                    || message.contains("transport error")
                    || message.contains("unknown ca"),
                "unexpected connection error: {error}"
            );
        }
    }

    stop_server(running)
        .await
        .expect("server should stop cleanly");
}

#[tokio::test]
async fn grpc_server_accepts_clients_with_certificate_in_mutual_mode() {
    let tls_fixture = TlsFixturePaths::new().expect("TLS fixture files should be written");
    let running = start_server(GrpcTlsConfig {
        mode: GrpcTlsMode::Mutual,
        cert_path: Some(tls_fixture.server_cert.clone()),
        key_path: Some(tls_fixture.server_key.clone()),
        client_ca_path: Some(tls_fixture.client_ca.clone()),
    })
    .await
    .expect("mTLS server should start");

    let channel = connect_tls_channel(&running.endpoint, true)
        .await
        .expect("mTLS client should connect with a certificate");
    let mut query_client = ContextQueryServiceClient::new(channel);
    let response = get_context(&mut query_client)
        .await
        .expect("mTLS roundtrip should succeed")
        .into_inner();

    assert_eq!(
        response.bundle.expect("bundle should exist").root_node_id,
        "case-123"
    );

    stop_server(running)
        .await
        .expect("server should stop cleanly");
}

struct RunningTlsServer {
    shutdown_tx: Option<oneshot::Sender<()>>,
    server_task: tokio::task::JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>>,
    endpoint: String,
}

async fn start_server(
    grpc_tls: GrpcTlsConfig,
) -> Result<RunningTlsServer, Box<dyn std::error::Error + Send + Sync>> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let server = GrpcServer::new(
        AppConfig {
            service_name: "rehydration-kernel".to_string(),
            grpc_bind: addr.to_string(),
            admin_bind: "127.0.0.1:8080".to_string(),
            grpc_tls,
            graph_uri: "neo4j://localhost:7687".to_string(),
            detail_uri: "redis://localhost:6379".to_string(),
            snapshot_uri: "redis://localhost:6379".to_string(),
            events_subject_prefix: "rehydration".to_string(),
        },
        EmptyGraphNeighborhoodReader,
        EmptyNodeDetailReader,
        NoopSnapshotStore,
    );
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let server_task = tokio::spawn(async move {
        server
            .serve_with_listener_shutdown(listener, async move {
                let _ = shutdown_rx.await;
            })
            .await
    });

    Ok(RunningTlsServer {
        shutdown_tx: Some(shutdown_tx),
        server_task,
        endpoint: format!("https://localhost:{}", addr.port()),
    })
}

async fn stop_server(
    server: RunningTlsServer,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let RunningTlsServer {
        shutdown_tx,
        server_task,
        endpoint: _,
    } = server;
    if let Some(tx) = shutdown_tx {
        let _ = tx.send(());
    }

    server_task.await?
}

async fn connect_tls_channel(
    endpoint: &str,
    include_client_identity: bool,
) -> Result<Channel, Box<dyn std::error::Error + Send + Sync>> {
    let mut tls = ClientTlsConfig::new()
        .ca_certificate(Certificate::from_pem(CA_CERT))
        .domain_name("localhost");
    if include_client_identity {
        tls = tls.identity(Identity::from_pem(CLIENT_CERT, CLIENT_KEY));
    }

    let endpoint = Endpoint::from_shared(endpoint.to_string())?.tls_config(tls)?;
    let mut attempts = 0u8;

    loop {
        match endpoint.clone().connect().await {
            Ok(channel) => return Ok(channel),
            Err(error) if attempts < 20 => {
                attempts += 1;
                sleep(std::time::Duration::from_millis(25)).await;
                if error.to_string().contains("Connection refused") {
                    continue;
                }
                return Err(Box::new(error));
            }
            Err(error) => return Err(Box::new(error)),
        }
    }
}

async fn get_context(
    client: &mut ContextQueryServiceClient<Channel>,
) -> Result<tonic::Response<rehydration_proto::v1alpha1::GetContextResponse>, tonic::Status> {
    client
        .get_context(GetContextRequest {
            root_node_id: "case-123".to_string(),
            role: "developer".to_string(),
            phase: Phase::Build as i32,
            work_item_id: String::new(),
            token_budget: 1024,
            requested_scopes: vec!["graph".to_string()],
            render_format: BundleRenderFormat::Structured as i32,
            include_debug_sections: false,
        })
        .await
}

fn write_fixture(dir: &TempDir, name: &str, contents: &str) -> io::Result<PathBuf> {
    let path = dir.path().join(name);
    fs::write(&path, contents)?;
    Ok(path)
}
