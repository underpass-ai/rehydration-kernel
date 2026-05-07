use rehydration_mcp::{
    GRPC_ENDPOINT_ENV, GRPC_TLS_CA_PATH_ENV, GRPC_TLS_CERT_PATH_ENV, GRPC_TLS_DOMAIN_NAME_ENV,
    GRPC_TLS_KEY_PATH_ENV, GRPC_TLS_MODE_ENV, KernelMcpServer, MCP_BACKEND_ENV,
};
use std::io::{self, BufRead, Write};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();

    let server = match KernelMcpServer::try_from_env() {
        Ok(server) => server,
        Err(message) => {
            eprintln!("rehydration-mcp: {message}");
            eprintln!(
                "rehydration-mcp: set {GRPC_ENDPOINT_ENV} for live gRPC, or set {MCP_BACKEND_ENV}=fixture explicitly for fixture mode"
            );
            std::process::exit(2);
        }
    };
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    if server.backend_name() == "grpc" {
        eprintln!(
            "rehydration-mcp: using live gRPC backend from {GRPC_ENDPOINT_ENV} with {GRPC_TLS_MODE_ENV}={}",
            server.grpc_tls_mode_name()
        );
        if server.grpc_tls_mode_name() != "disabled" {
            eprintln!(
                "rehydration-mcp: TLS envs: {GRPC_TLS_CA_PATH_ENV}, {GRPC_TLS_CERT_PATH_ENV}, {GRPC_TLS_KEY_PATH_ENV}, {GRPC_TLS_DOMAIN_NAME_ENV}"
            );
        }
    } else {
        eprintln!("rehydration-mcp: using explicit fixture backend");
    }

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        if let Some(response) = server.handle_json_line(&line).await {
            writeln!(stdout, "{response}")?;
            stdout.flush()?;
        }
    }

    Ok(())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("rehydration_mcp=info"));
    tracing_subscriber::fmt()
        .json()
        .with_writer(io::stderr)
        .with_env_filter(filter)
        .init();
}
