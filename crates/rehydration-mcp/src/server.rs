use std::sync::Arc;

use serde_json::Value;

use crate::backend::{
    GRPC_ENDPOINT_ENV, KernelMcpGrpcTlsConfig, KernelMcpToolBackend, KernelMcpToolFuture,
    MCP_BACKEND_ENV,
};
use crate::fixture::FixtureKernelMcpBackend;
use crate::grpc::GrpcKernelMcpBackend;
use crate::protocol::{
    initialize_result, jsonrpc_error, jsonrpc_result, tool_error_result, tools_list_result,
};

pub struct KernelMcpServer {
    backend: Arc<dyn KernelMcpToolBackend>,
}

impl Default for KernelMcpServer {
    fn default() -> Self {
        Self::fixture()
    }
}

impl KernelMcpServer {
    pub fn fixture() -> Self {
        Self::with_backend(FixtureKernelMcpBackend)
    }

    pub fn grpc(endpoint: impl Into<String>) -> Self {
        Self::grpc_with_tls(endpoint, KernelMcpGrpcTlsConfig::disabled())
    }

    pub fn grpc_with_tls(endpoint: impl Into<String>, tls: KernelMcpGrpcTlsConfig) -> Self {
        Self::with_backend(GrpcKernelMcpBackend::new(endpoint, tls))
    }

    pub fn with_backend(backend: impl KernelMcpToolBackend + 'static) -> Self {
        Self {
            backend: Arc::new(backend),
        }
    }

    pub fn with_shared_backend(backend: Arc<dyn KernelMcpToolBackend>) -> Self {
        Self { backend }
    }

    pub fn from_env() -> Self {
        Self::try_from_env().expect("valid MCP backend configuration")
    }

    pub fn try_from_env() -> Result<Self, String> {
        let backend = std::env::var(MCP_BACKEND_ENV)
            .ok()
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "grpc".to_string());
        let endpoint = std::env::var(GRPC_ENDPOINT_ENV).ok();
        let tls = KernelMcpGrpcTlsConfig::from_env_for_endpoint(endpoint.as_deref());

        match backend.as_str() {
            "grpc" | "live" => {
                let Some(endpoint) = endpoint.filter(|endpoint| !endpoint.trim().is_empty()) else {
                    return Err(format!(
                        "{GRPC_ENDPOINT_ENV} is required when {MCP_BACKEND_ENV}=grpc"
                    ));
                };
                Ok(Self::grpc_with_tls(endpoint, tls))
            }
            "fixture" | "fixtures" => Ok(Self::fixture()),
            other => Err(format!(
                "unsupported {MCP_BACKEND_ENV} value `{other}`; use `grpc` or `fixture`"
            )),
        }
    }

    pub fn from_optional_endpoint(endpoint: Option<String>) -> Self {
        Self::from_optional_endpoint_and_tls(endpoint, KernelMcpGrpcTlsConfig::disabled())
    }

    pub fn from_optional_endpoint_and_tls(
        endpoint: Option<String>,
        tls: KernelMcpGrpcTlsConfig,
    ) -> Self {
        match endpoint.filter(|endpoint| !endpoint.trim().is_empty()) {
            Some(endpoint) => Self::grpc_with_tls(endpoint, tls),
            None => Self::fixture(),
        }
    }

    pub fn backend_name(&self) -> &'static str {
        self.backend.backend_name()
    }

    pub fn grpc_tls_mode_name(&self) -> &'static str {
        self.backend.grpc_tls_mode_name()
    }

    pub async fn handle_json_line(&self, line: &str) -> Option<String> {
        let request = match serde_json::from_str::<Value>(line) {
            Ok(request) => request,
            Err(error) => {
                return Some(jsonrpc_error(
                    Value::Null,
                    -32700,
                    &format!("invalid JSON-RPC message: {error}"),
                ));
            }
        };

        let id = request.get("id").cloned();
        let method = request.get("method").and_then(Value::as_str);

        match method {
            Some("initialize") => id.map(|id| {
                jsonrpc_result(
                    id,
                    initialize_result(self.backend_name(), self.grpc_tls_mode_name()),
                )
            }),
            Some("notifications/initialized") => None,
            Some("tools/list") => id.map(|id| jsonrpc_result(id, tools_list_result())),
            Some("tools/call") => match id {
                Some(id) => Some(self.handle_tool_call(id, request.get("params")).await),
                None => None,
            },
            Some(other) => id.map(|id| {
                jsonrpc_error(
                    id,
                    -32601,
                    &format!("unsupported JSON-RPC method `{other}`"),
                )
            }),
            None => Some(jsonrpc_error(
                Value::Null,
                -32600,
                "missing JSON-RPC method",
            )),
        }
    }

    async fn handle_tool_call(&self, id: Value, params: Option<&Value>) -> String {
        let Some(params) = params.and_then(Value::as_object) else {
            return jsonrpc_error(id, -32602, "tools/call requires object params");
        };
        let Some(name) = params.get("name").and_then(Value::as_str) else {
            return jsonrpc_error(id, -32602, "tools/call requires params.name");
        };
        let arguments = params.get("arguments").unwrap_or(&Value::Null);

        match self.backend.call_tool(name, arguments).await {
            Ok(result) => jsonrpc_result(id, result),
            Err(message) => jsonrpc_result(id, tool_error_result(&message)),
        }
    }
}

impl<T> KernelMcpToolBackend for Arc<T>
where
    T: KernelMcpToolBackend + ?Sized,
{
    fn backend_name(&self) -> &'static str {
        self.as_ref().backend_name()
    }

    fn grpc_tls_mode_name(&self) -> &'static str {
        self.as_ref().grpc_tls_mode_name()
    }

    fn call_tool<'a>(&'a self, name: &'a str, arguments: &'a Value) -> KernelMcpToolFuture<'a> {
        self.as_ref().call_tool(name, arguments)
    }
}
