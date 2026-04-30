use rehydration_mcp::{GRPC_ENDPOINT_ENV, KernelMcpServer};
use std::io::{self, BufRead, Write};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = KernelMcpServer::from_env();
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    if server.backend_name() == "grpc" {
        eprintln!("rehydration-mcp: using live gRPC backend from {GRPC_ENDPOINT_ENV}");
    } else {
        eprintln!("rehydration-mcp: using fixture backend; set {GRPC_ENDPOINT_ENV} for live reads");
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
