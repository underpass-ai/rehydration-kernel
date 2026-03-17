use std::fs::File;
use std::io::BufReader as StdBufReader;
use std::net::IpAddr;
use std::path::Path;
use std::sync::{Arc, Once};

use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use tokio_rustls::rustls::{ClientConfig, RootCertStore};

use rehydration_ports::PortError;

use crate::adapter::endpoint::ValkeyEndpoint;
use crate::adapter::resp::{
    RespValue, encode_command, encode_set_command, map_valkey_response, read_response,
};

trait ValkeyIo: AsyncRead + AsyncWrite + Unpin + Send {}

impl<T> ValkeyIo for T where T: AsyncRead + AsyncWrite + Unpin + Send {}

type BoxedValkeyIo = Box<dyn ValkeyIo>;

pub(crate) async fn execute_set_command(
    endpoint: &ValkeyEndpoint,
    key: &str,
    payload: &str,
    ttl_seconds: Option<u64>,
) -> Result<(), PortError> {
    let mut stream = connect_stream(endpoint).await?;

    let frame = encode_set_command(key, payload, ttl_seconds.or(endpoint.ttl_seconds));
    stream.write_all(&frame).await.map_err(|error| {
        PortError::Unavailable(format!("failed to write valkey payload: {error}"))
    })?;
    stream.flush().await.map_err(|error| {
        PortError::Unavailable(format!("failed to flush valkey payload: {error}"))
    })?;

    let mut reader = BufReader::new(stream);
    map_valkey_response(read_response(&mut reader).await?)
}

pub(crate) async fn execute_get_command(
    endpoint: &ValkeyEndpoint,
    key: &str,
) -> Result<Option<String>, PortError> {
    let mut stream = connect_stream(endpoint).await?;

    let frame = encode_command(&["GET", key]);
    stream.write_all(&frame).await.map_err(|error| {
        PortError::Unavailable(format!("failed to write valkey command: {error}"))
    })?;
    stream.flush().await.map_err(|error| {
        PortError::Unavailable(format!("failed to flush valkey command: {error}"))
    })?;

    let mut reader = BufReader::new(stream);
    match read_response(&mut reader).await? {
        RespValue::BulkString(payload) => Ok(payload),
        RespValue::Error(message) => Err(PortError::Unavailable(format!(
            "valkey rejected read: {message}"
        ))),
        other => Err(PortError::Unavailable(format!(
            "unexpected valkey response: {other:?}"
        ))),
    }
}

async fn connect_stream(endpoint: &ValkeyEndpoint) -> Result<BoxedValkeyIo, PortError> {
    let stream = TcpStream::connect(endpoint.address())
        .await
        .map_err(|error| {
            PortError::Unavailable(format!(
                "unable to connect to valkey {}: {error}",
                endpoint.raw_uri
            ))
        })?;

    if !endpoint.tls.enabled {
        return Ok(Box::new(stream));
    }

    let connector = TlsConnector::from(Arc::new(build_tls_client_config(endpoint)?));
    let server_name = parse_server_name(endpoint)?;
    let stream = connector
        .connect(server_name, stream)
        .await
        .map_err(|error| {
            PortError::Unavailable(format!(
                "unable to establish valkey TLS for {}: {error}",
                endpoint.raw_uri
            ))
        })?;

    Ok(Box::new(stream))
}

fn build_tls_client_config(endpoint: &ValkeyEndpoint) -> Result<ClientConfig, PortError> {
    ensure_crypto_provider();
    let roots = load_root_certificates(endpoint)?;
    let builder = ClientConfig::builder().with_root_certificates(roots);

    match (&endpoint.tls.cert_path, &endpoint.tls.key_path) {
        (Some(cert_path), Some(key_path)) => {
            let certs = load_pem_certificates(cert_path)?;
            let key = load_private_key(key_path)?;
            builder.with_client_auth_cert(certs, key).map_err(|error| {
                PortError::InvalidState(format!(
                    "unable to configure valkey client identity for {}: {error}",
                    endpoint.raw_uri
                ))
            })
        }
        (None, None) => Ok(builder.with_no_client_auth()),
        _ => Err(PortError::InvalidState(format!(
            "valkey TLS client certificate and key must be configured together for {}",
            endpoint.raw_uri
        ))),
    }
}

fn load_root_certificates(endpoint: &ValkeyEndpoint) -> Result<RootCertStore, PortError> {
    let mut roots = RootCertStore::empty();

    if let Some(ca_path) = &endpoint.tls.ca_path {
        for cert in load_pem_certificates(ca_path)? {
            roots.add(cert).map_err(|error| {
                PortError::InvalidState(format!(
                    "unable to load valkey CA certificate `{}`: {error}",
                    ca_path.display()
                ))
            })?;
        }
        return Ok(roots);
    }

    let native = rustls_native_certs::load_native_certs();
    for cert in native.certs {
        roots.add(cert).map_err(|error| {
            PortError::InvalidState(format!(
                "unable to load system trust store for valkey TLS: {error}"
            ))
        })?;
    }

    if roots.is_empty() {
        return Err(PortError::InvalidState(
            "no system root certificates available for valkey TLS; configure tls_ca_path"
                .to_string(),
        ));
    }

    Ok(roots)
}

fn load_pem_certificates(path: &Path) -> Result<Vec<CertificateDer<'static>>, PortError> {
    let file = File::open(path).map_err(|error| {
        PortError::InvalidState(format!(
            "unable to open valkey certificate file `{}`: {error}",
            path.display()
        ))
    })?;

    rustls_pemfile::certs(&mut StdBufReader::new(file))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            PortError::InvalidState(format!(
                "unable to read valkey certificate file `{}`: {error}",
                path.display()
            ))
        })
}

fn load_private_key(path: &Path) -> Result<PrivateKeyDer<'static>, PortError> {
    let file = File::open(path).map_err(|error| {
        PortError::InvalidState(format!(
            "unable to open valkey private key file `{}`: {error}",
            path.display()
        ))
    })?;

    rustls_pemfile::private_key(&mut StdBufReader::new(file))
        .map_err(|error| {
            PortError::InvalidState(format!(
                "unable to read valkey private key file `{}`: {error}",
                path.display()
            ))
        })?
        .ok_or_else(|| {
            PortError::InvalidState(format!(
                "valkey private key file `{}` does not contain a private key",
                path.display()
            ))
        })
}

fn parse_server_name(endpoint: &ValkeyEndpoint) -> Result<ServerName<'static>, PortError> {
    let host = endpoint.server_name();
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(ServerName::IpAddress(ip.into()));
    }

    ServerName::try_from(host.to_string()).map_err(|error| {
        PortError::InvalidState(format!(
            "invalid valkey TLS server name `{host}` for {}: {error}",
            endpoint.raw_uri
        ))
    })
}

fn ensure_crypto_provider() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let _ = tokio_rustls::rustls::crypto::aws_lc_rs::default_provider().install_default();
    });
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io;
    use std::path::Path;
    use std::path::PathBuf;
    use std::process::Command;
    use std::sync::Arc;

    use tempfile::TempDir;
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
    use tokio::net::TcpListener;
    use tokio::sync::oneshot;
    use tokio_rustls::TlsAcceptor;
    use tokio_rustls::rustls::RootCertStore;
    use tokio_rustls::rustls::ServerConfig;
    use tokio_rustls::rustls::server::WebPkiClientVerifier;

    use super::{
        build_tls_client_config, ensure_crypto_provider, execute_get_command, execute_set_command,
        load_pem_certificates, load_private_key,
    };
    use crate::adapter::endpoint::{ValkeyEndpoint, ValkeyTlsConfig};
    use rehydration_ports::PortError;

    #[tokio::test]
    async fn execute_commands_support_rediss() {
        let tls = TlsFixturePaths::new().expect("TLS fixture files should be written");
        let server = spawn_tls_server(&tls, false)
            .await
            .expect("TLS valkey server should start");
        let endpoint = ValkeyEndpoint {
            raw_uri: format!(
                "rediss://127.0.0.1:{}?tls_ca_path={}",
                server.port,
                tls.ca_cert.display()
            ),
            host: "127.0.0.1".to_string(),
            port: server.port,
            key_prefix: "rehydration:test".to_string(),
            ttl_seconds: None,
            tls: ValkeyTlsConfig {
                enabled: true,
                ca_path: Some(tls.ca_cert.clone()),
                cert_path: None,
                key_path: None,
            },
        };

        execute_set_command(&endpoint, "rehydration:test:node-123", "payload", Some(5))
            .await
            .expect("TLS write should succeed");
        let payload = execute_get_command(&endpoint, "rehydration:test:node-123")
            .await
            .expect("TLS read should succeed");

        assert_eq!(payload, Some("payload".to_string()));

        stop_tls_server(server)
            .await
            .expect("TLS server should stop cleanly");
    }

    #[test]
    fn tls_client_config_loads_client_identity() {
        let tls = TlsFixturePaths::new().expect("TLS fixture files should be written");
        let endpoint = ValkeyEndpoint {
            raw_uri: format!(
                "rediss://localhost:6379?tls_ca_path={}&tls_cert_path={}&tls_key_path={}",
                tls.ca_cert.display(),
                tls.client_cert.display(),
                tls.client_key.display()
            ),
            host: "localhost".to_string(),
            port: 6379,
            key_prefix: "rehydration:test".to_string(),
            ttl_seconds: None,
            tls: ValkeyTlsConfig {
                enabled: true,
                ca_path: Some(tls.ca_cert.clone()),
                cert_path: Some(tls.client_cert.clone()),
                key_path: Some(tls.client_key.clone()),
            },
        };

        build_tls_client_config(&endpoint).expect("client identity should load");
    }

    struct TlsFixturePaths {
        _dir: TempDir,
        ca_cert: PathBuf,
        server_cert: PathBuf,
        server_key: PathBuf,
        client_cert: PathBuf,
        client_key: PathBuf,
    }

    impl TlsFixturePaths {
        fn new() -> io::Result<Self> {
            let dir = tempfile::tempdir()?;
            let ca_key = dir.path().join("ca.key");
            let ca_cert = dir.path().join("ca.crt");
            let server_key = dir.path().join("server.key");
            let server_csr = dir.path().join("server.csr");
            let server_ext = dir.path().join("server.ext");
            let server_cert = dir.path().join("server.crt");
            let client_key = dir.path().join("client.key");
            let client_csr = dir.path().join("client.csr");
            let client_ext = dir.path().join("client.ext");
            let client_cert = dir.path().join("client.crt");

            run_openssl([
                "req",
                "-x509",
                "-newkey",
                "rsa:2048",
                "-days",
                "3650",
                "-nodes",
                "-keyout",
                &path_string(&ca_key)?,
                "-out",
                &path_string(&ca_cert)?,
                "-subj",
                "/CN=rehydration-valkey-test-ca",
            ])?;

            run_openssl([
                "req",
                "-newkey",
                "rsa:2048",
                "-nodes",
                "-keyout",
                &path_string(&server_key)?,
                "-out",
                &path_string(&server_csr)?,
                "-subj",
                "/CN=localhost",
                "-addext",
                "subjectAltName=DNS:localhost,IP:127.0.0.1",
            ])?;
            fs::write(
                &server_ext,
                "[v3_req]\nsubjectAltName=DNS:localhost,IP:127.0.0.1\nextendedKeyUsage=serverAuth\n",
            )?;
            run_openssl([
                "x509",
                "-req",
                "-in",
                &path_string(&server_csr)?,
                "-CA",
                &path_string(&ca_cert)?,
                "-CAkey",
                &path_string(&ca_key)?,
                "-CAcreateserial",
                "-out",
                &path_string(&server_cert)?,
                "-days",
                "3650",
                "-extfile",
                &path_string(&server_ext)?,
                "-extensions",
                "v3_req",
            ])?;

            run_openssl([
                "req",
                "-newkey",
                "rsa:2048",
                "-nodes",
                "-keyout",
                &path_string(&client_key)?,
                "-out",
                &path_string(&client_csr)?,
                "-subj",
                "/CN=rehydration-valkey-test-client",
            ])?;
            fs::write(&client_ext, "[v3_req]\nextendedKeyUsage=clientAuth\n")?;
            run_openssl([
                "x509",
                "-req",
                "-in",
                &path_string(&client_csr)?,
                "-CA",
                &path_string(&ca_cert)?,
                "-CAkey",
                &path_string(&ca_key)?,
                "-CAcreateserial",
                "-out",
                &path_string(&client_cert)?,
                "-days",
                "3650",
                "-extfile",
                &path_string(&client_ext)?,
                "-extensions",
                "v3_req",
            ])?;

            Ok(Self {
                _dir: dir,
                ca_cert,
                server_cert,
                server_key,
                client_cert,
                client_key,
            })
        }
    }

    struct RunningTlsServer {
        port: u16,
        shutdown_tx: Option<oneshot::Sender<()>>,
        task: tokio::task::JoinHandle<io::Result<()>>,
    }

    async fn spawn_tls_server(
        fixture: &TlsFixturePaths,
        require_client_identity: bool,
    ) -> io::Result<RunningTlsServer> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();
        let acceptor = TlsAcceptor::from(Arc::new(build_server_tls_config(
            fixture,
            require_client_identity,
        )?));
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();

        let task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut shutdown_rx => return Ok(()),
                    accepted = listener.accept() => {
                        let (socket, _) = accepted?;
                        let acceptor = acceptor.clone();
                        tokio::spawn(async move {
                            let mut tls_stream = acceptor.accept(socket).await.expect("TLS handshake should succeed");
                            let mut header = String::new();
                            let mut reader = BufReader::new(&mut tls_stream);
                            reader.read_line(&mut header).await.expect("RESP header should be read");
                            let argc = header.trim_end_matches("\r\n").trim_start_matches('*').parse::<usize>().expect("argc should parse");
                            let mut args = Vec::with_capacity(argc);
                            for _ in 0..argc {
                                let mut length = String::new();
                                reader.read_line(&mut length).await.expect("bulk header should be read");
                                let size = length.trim_end_matches("\r\n").trim_start_matches('$').parse::<usize>().expect("bulk size should parse");
                                let mut bytes = vec![0_u8; size];
                                reader.read_exact(&mut bytes).await.expect("bulk payload should be read");
                                let mut crlf = [0_u8; 2];
                                reader.read_exact(&mut crlf).await.expect("RESP terminator should be read");
                                args.push(String::from_utf8(bytes).expect("UTF-8 payload"));
                            }

                            let response = match args.first().map(String::as_str) {
                                Some("SET") => b"+OK\r\n".to_vec(),
                                Some("GET") => b"$7\r\npayload\r\n".to_vec(),
                                other => panic!("unexpected command: {other:?}"),
                            };
                            tls_stream.write_all(&response).await.expect("response should write");
                            tls_stream.flush().await.expect("response should flush");
                        });
                    }
                }
            }
        });

        Ok(RunningTlsServer {
            port,
            shutdown_tx: Some(shutdown_tx),
            task,
        })
    }

    async fn stop_tls_server(server: RunningTlsServer) -> io::Result<()> {
        let RunningTlsServer {
            port: _,
            shutdown_tx,
            task,
        } = server;
        if let Some(tx) = shutdown_tx {
            let _ = tx.send(());
        }
        task.await.expect("join should succeed")
    }

    fn build_server_tls_config(
        fixture: &TlsFixturePaths,
        require_client_identity: bool,
    ) -> io::Result<ServerConfig> {
        ensure_crypto_provider();
        let certs = load_pem_certificates(&fixture.server_cert).map_err(port_error_to_io)?;
        let key = load_private_key(&fixture.server_key).map_err(port_error_to_io)?;
        let builder = ServerConfig::builder();

        if require_client_identity {
            let mut roots = RootCertStore::empty();
            for cert in load_pem_certificates(&fixture.ca_cert).map_err(port_error_to_io)? {
                roots
                    .add(cert)
                    .map_err(|error| io::Error::other(error.to_string()))?;
            }
            let verifier = WebPkiClientVerifier::builder(Arc::new(roots))
                .build()
                .map_err(|error| io::Error::other(error.to_string()))?;

            return builder
                .with_client_cert_verifier(verifier)
                .with_single_cert(certs, key)
                .map_err(|error| io::Error::other(error.to_string()));
        }

        builder
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .map_err(|error| io::Error::other(error.to_string()))
    }

    fn port_error_to_io(error: PortError) -> io::Error {
        io::Error::other(error.to_string())
    }

    fn path_string(path: &Path) -> io::Result<String> {
        path.to_str()
            .map(ToString::to_string)
            .ok_or_else(|| io::Error::other(format!("path is not valid UTF-8: {}", path.display())))
    }

    fn run_openssl<const N: usize>(args: [&str; N]) -> io::Result<()> {
        let output = Command::new("openssl").args(args).output()?;
        if output.status.success() {
            return Ok(());
        }

        Err(io::Error::other(format!(
            "openssl command failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }
}
