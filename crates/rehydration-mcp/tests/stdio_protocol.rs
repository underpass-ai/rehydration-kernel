use rehydration_mcp::{KernelMcpGrpcTlsConfig, KernelMcpServer};
use serde_json::{Value, json};

#[test]
fn backend_selection_defaults_to_fixtures_without_endpoint() {
    let server = KernelMcpServer::from_optional_endpoint(None);
    assert_eq!(server.backend_name(), "fixture");
}

#[test]
fn backend_selection_uses_grpc_when_endpoint_is_present() {
    let server =
        KernelMcpServer::from_optional_endpoint(Some("http://127.0.0.1:50051".to_string()));
    assert_eq!(server.backend_name(), "grpc");
}

#[test]
fn backend_selection_reports_grpc_tls_mode() {
    let server = KernelMcpServer::grpc_with_tls(
        "https://rehydration-kernel.underpassai.com",
        KernelMcpGrpcTlsConfig::server("/tmp/ca.crt", None),
    );

    assert_eq!(server.backend_name(), "grpc");
    assert_eq!(server.grpc_tls_mode_name(), "server");
}

#[tokio::test]
async fn initialize_declares_tools_capability() {
    let response = handle(json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {}
    }))
    .await;

    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], 1);
    assert_eq!(
        response["result"]["serverInfo"]["name"],
        "rehydration-kernel-kmp"
    );
    assert_eq!(response["result"]["metadata"]["backend"], "fixture");
    assert!(response["result"]["capabilities"].get("tools").is_some());
}

#[tokio::test]
async fn tools_list_exposes_read_only_kmp_tools() {
    let response = handle(json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
        "params": {}
    }))
    .await;

    let tool_names = response["result"]["tools"]
        .as_array()
        .expect("tools should be an array")
        .iter()
        .map(|tool| tool["name"].as_str().expect("tool should have a name"))
        .collect::<Vec<_>>();

    assert_eq!(
        tool_names,
        vec![
            "kernel_wake",
            "kernel_ask",
            "kernel_trace",
            "kernel_inspect"
        ]
    );
}

#[tokio::test]
async fn kernel_ask_returns_fixture_backed_structured_content() {
    let response = handle(json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "kernel_ask",
            "arguments": {
                "about": "question:830ce83f",
                "question": "Where did Rachel move after her recent relocation?",
                "answer_policy": "evidence_or_unknown"
            }
        }
    }))
    .await;

    assert_eq!(response["result"]["isError"], false);
    assert_eq!(response["result"]["structuredContent"]["answer"], "Austin");
    assert_eq!(
        response["result"]["structuredContent"]["proof"]["confidence"],
        "high"
    );
}

#[tokio::test]
async fn grpc_backend_returns_tool_error_when_live_kernel_is_unavailable() {
    let server = KernelMcpServer::grpc("http://127.0.0.1:1");
    let response = handle_with(
        &server,
        json!({
            "jsonrpc": "2.0",
            "id": 31,
            "method": "tools/call",
            "params": {
                "name": "kernel_inspect",
                "arguments": {
                    "ref": "node:missing"
                }
            }
        }),
    )
    .await;

    assert_eq!(response["result"]["isError"], true);
    assert!(
        response["result"]["content"][0]["text"]
            .as_str()
            .expect("tool error should include text")
            .contains("failed to connect to kernel gRPC endpoint")
    );
}

#[tokio::test]
async fn invalid_tool_arguments_return_tool_error() {
    let response = handle(json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "tools/call",
        "params": {
            "name": "kernel_trace",
            "arguments": {
                "from": "claim:rachel-austin"
            }
        }
    }))
    .await;

    assert_eq!(response["result"]["isError"], true);
    assert!(
        response["result"]["content"][0]["text"]
            .as_str()
            .expect("tool error should include text")
            .contains("missing required argument `to`")
    );
}

#[tokio::test]
async fn initialized_notification_has_no_response() {
    let server = KernelMcpServer::fixture();
    let response = server
        .handle_json_line(
            &json!({
                "jsonrpc": "2.0",
                "method": "notifications/initialized",
                "params": {}
            })
            .to_string(),
        )
        .await;

    assert!(response.is_none());
}

async fn handle(request: Value) -> Value {
    let server = KernelMcpServer::fixture();
    handle_with(&server, request).await
}

async fn handle_with(server: &KernelMcpServer, request: Value) -> Value {
    let response = server
        .handle_json_line(&request.to_string())
        .await
        .expect("request should produce a response");
    serde_json::from_str(&response).expect("response should be JSON")
}
