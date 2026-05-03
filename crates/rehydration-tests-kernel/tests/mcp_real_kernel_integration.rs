#![cfg(feature = "container-tests")]

mod support;

use std::error::Error;

use rehydration_mcp::KernelMcpServer;
use rehydration_tests_shared::seed::kernel_data::{
    DECISION_DETAIL, DECISION_ID, DECISION_KIND, DEVELOPER_ROLE, HAS_TASK_RELATION,
    RECORDS_RELATION, ROOT_NODE_ID, TASK_ID,
};
use serde_json::{Value, json};

use crate::support::seeded_kernel_fixture::SeededKernelFixture;

#[tokio::test]
async fn mcp_tools_read_from_live_kernel_grpc_server() -> Result<(), Box<dyn Error + Send + Sync>> {
    let fixture = SeededKernelFixture::start().await?;

    let result = async {
        let server = KernelMcpServer::grpc(fixture.grpc_endpoint().to_string());

        let initialize = call_json_rpc(
            &server,
            1,
            "initialize",
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "kernel-container-smoke",
                    "version": "0.1.0"
                }
            }),
        )
        .await;
        assert_eq!(
            initialize.pointer("/result/metadata/backend"),
            Some(&Value::String("grpc".to_string()))
        );

        let wake = call_tool(
            &server,
            2,
            "kernel_wake",
            json!({
                "about": ROOT_NODE_ID,
                "role": DEVELOPER_ROLE,
                "intent": "continue from the seeded kernel memory",
                "depth": 2,
                "budget": {
                    "tokens": 2048
                }
            }),
        )
        .await;
        assert_tool_success(&wake);
        let wake_content = structured_content(&wake);
        assert_non_empty_string(wake_content, "/summary");
        assert_non_empty_array(wake_content, "/wake/current_state");
        assert_array_contains_relation(
            wake_content,
            "/proof/path",
            ROOT_NODE_ID,
            DECISION_ID,
            RECORDS_RELATION,
        );
        assert_array_contains_relation(
            wake_content,
            "/proof/path",
            ROOT_NODE_ID,
            TASK_ID,
            HAS_TASK_RELATION,
        );
        assert_array_contains_evidence(wake_content, "/proof/evidence", ROOT_NODE_ID);
        assert_array_contains_evidence(wake_content, "/proof/evidence", DECISION_ID);

        let ask = call_tool(
            &server,
            3,
            "kernel_ask",
            json!({
                "about": ROOT_NODE_ID,
                "question": "Which seeded decision should the next agent inspect?",
                "depth": 2,
                "budget": {
                    "tokens": 2048
                }
            }),
        )
        .await;
        assert_tool_success(&ask);
        let ask_content = structured_content(&ask);
        assert_eq!(ask_content.pointer("/answer"), Some(&Value::Null));
        assert_array_contains_evidence(ask_content, "/proof/evidence", DECISION_ID);
        assert!(
            array_at(ask_content, "/proof/missing")
                .iter()
                .any(|value| value.as_str() == Some("generative_answer")),
            "kernel_ask should stay honest about the missing generative answer"
        );

        let trace = call_tool(
            &server,
            4,
            "kernel_trace",
            json!({
                "from": ROOT_NODE_ID,
                "to": TASK_ID,
                "role": DEVELOPER_ROLE,
                "budget": {
                    "tokens": 1024
                }
            }),
        )
        .await;
        assert_tool_success(&trace);
        let trace_content = structured_content(&trace);
        assert_non_empty_string(trace_content, "/summary");
        assert_array_contains_relation(
            trace_content,
            "/trace",
            ROOT_NODE_ID,
            TASK_ID,
            HAS_TASK_RELATION,
        );

        let inspect = call_tool(
            &server,
            5,
            "kernel_inspect",
            json!({
                "ref": DECISION_ID,
                "include": {
                    "details": true
                }
            }),
        )
        .await;
        assert_tool_success(&inspect);
        let inspect_content = structured_content(&inspect);
        assert_eq!(
            inspect_content.pointer("/object/ref"),
            Some(&Value::String(DECISION_ID.to_string()))
        );
        assert_eq!(
            inspect_content.pointer("/object/kind"),
            Some(&Value::String(DECISION_KIND.to_string()))
        );
        assert!(
            array_at(inspect_content, "/evidence")
                .iter()
                .any(|value| value.get("text").and_then(Value::as_str) == Some(DECISION_DETAIL)),
            "kernel_inspect should expose the live Valkey detail"
        );

        Ok::<(), Box<dyn Error + Send + Sync>>(())
    }
    .await;

    fixture.shutdown().await?;
    result
}

async fn call_tool(server: &KernelMcpServer, id: u64, name: &str, arguments: Value) -> Value {
    call_json_rpc(
        server,
        id,
        "tools/call",
        json!({
            "name": name,
            "arguments": arguments
        }),
    )
    .await
}

async fn call_json_rpc(server: &KernelMcpServer, id: u64, method: &str, params: Value) -> Value {
    let response = server
        .handle_json_line(
            &json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": method,
                "params": params
            })
            .to_string(),
        )
        .await
        .expect("JSON-RPC request should produce a response");

    serde_json::from_str(&response).expect("JSON-RPC response should be valid JSON")
}

fn assert_tool_success(response: &Value) {
    assert_eq!(
        response.pointer("/result/isError"),
        Some(&Value::Bool(false))
    );
    assert!(
        response.pointer("/result/structuredContent").is_some(),
        "successful MCP tool response should include structuredContent"
    );
}

fn structured_content(response: &Value) -> &Value {
    response
        .pointer("/result/structuredContent")
        .expect("MCP response should include structuredContent")
}

fn assert_non_empty_string(value: &Value, pointer: &str) {
    assert!(
        value
            .pointer(pointer)
            .and_then(Value::as_str)
            .map(|text| !text.trim().is_empty())
            .unwrap_or(false),
        "{pointer} should be a non-empty string"
    );
}

fn assert_non_empty_array(value: &Value, pointer: &str) {
    assert!(
        !array_at(value, pointer).is_empty(),
        "{pointer} should be a non-empty array"
    );
}

fn array_at<'a>(value: &'a Value, pointer: &str) -> &'a [Value] {
    value
        .pointer(pointer)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .expect("JSON pointer should resolve to an array")
}

fn assert_array_contains_relation(value: &Value, pointer: &str, from: &str, to: &str, rel: &str) {
    assert!(
        array_at(value, pointer).iter().any(|entry| {
            entry.get("from").and_then(Value::as_str) == Some(from)
                && entry.get("to").and_then(Value::as_str) == Some(to)
                && entry.get("rel").and_then(Value::as_str) == Some(rel)
        }),
        "{pointer} should contain relation {from} -[{rel}]-> {to}"
    );
}

fn assert_array_contains_evidence(value: &Value, pointer: &str, source: &str) {
    assert!(
        array_at(value, pointer)
            .iter()
            .any(|entry| entry.get("source").and_then(Value::as_str) == Some(source)),
        "{pointer} should contain evidence from {source}"
    );
}
