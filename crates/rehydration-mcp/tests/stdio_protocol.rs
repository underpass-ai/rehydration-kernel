use rehydration_mcp::KernelMcpServer;
use serde_json::{Value, json};

#[test]
fn initialize_declares_tools_capability() {
    let response = handle(json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {}
    }));

    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], 1);
    assert_eq!(
        response["result"]["serverInfo"]["name"],
        "rehydration-kernel-kmp"
    );
    assert!(response["result"]["capabilities"].get("tools").is_some());
}

#[test]
fn tools_list_exposes_read_only_kmp_tools() {
    let response = handle(json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
        "params": {}
    }));

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

#[test]
fn kernel_ask_returns_fixture_backed_structured_content() {
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
    }));

    assert_eq!(response["result"]["isError"], false);
    assert_eq!(response["result"]["structuredContent"]["answer"], "Austin");
    assert_eq!(
        response["result"]["structuredContent"]["proof"]["confidence"],
        "high"
    );
}

#[test]
fn invalid_tool_arguments_return_tool_error() {
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
    }));

    assert_eq!(response["result"]["isError"], true);
    assert!(
        response["result"]["content"][0]["text"]
            .as_str()
            .expect("tool error should include text")
            .contains("missing required argument `to`")
    );
}

#[test]
fn initialized_notification_has_no_response() {
    let server = KernelMcpServer;
    let response = server.handle_json_line(
        &json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
            "params": {}
        })
        .to_string(),
    );

    assert!(response.is_none());
}

fn handle(request: Value) -> Value {
    let server = KernelMcpServer;
    let response = server
        .handle_json_line(&request.to_string())
        .expect("request should produce a response");
    serde_json::from_str(&response).expect("response should be JSON")
}
