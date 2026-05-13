mod args;
mod backend;
mod fixture;
mod grpc;
mod ingest;
mod kmp;
mod observability;
mod protocol;
mod server;
mod write;

pub use backend::{
    GRPC_ENDPOINT_ENV, GRPC_TLS_CA_PATH_ENV, GRPC_TLS_CERT_PATH_ENV, GRPC_TLS_DOMAIN_NAME_ENV,
    GRPC_TLS_KEY_PATH_ENV, GRPC_TLS_MODE_ENV, KernelMcpBackend, KernelMcpGrpcTlsConfig,
    KernelMcpGrpcTlsMode, KernelMcpToolBackend, KernelMcpToolFuture, MCP_BACKEND_ENV,
};
pub use fixture::FixtureKernelMcpBackend;
pub use grpc::GrpcKernelMcpBackend;
pub use server::KernelMcpServer;

pub fn kernel_mcp_tools_list_result() -> serde_json::Value {
    protocol::tools_list_result()
}

pub fn kernel_mcp_tool_names() -> Vec<String> {
    kernel_mcp_tools_list_result()
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|tool| tool.get("name").and_then(serde_json::Value::as_str))
        .map(ToString::to_string)
        .collect()
}
