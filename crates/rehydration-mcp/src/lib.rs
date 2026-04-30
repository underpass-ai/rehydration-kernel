use serde_json::{Value, json};

const PROTOCOL_VERSION: &str = "2024-11-05";
const SERVER_NAME: &str = "rehydration-kernel-kmp";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

const WAKE_RESPONSE_FIXTURE: &str =
    include_str!("../../../api/examples/kernel/v1beta1/kmp/wake.response.json");
const ASK_RESPONSE_FIXTURE: &str =
    include_str!("../../../api/examples/kernel/v1beta1/kmp/ask.response.json");
const TRACE_RESPONSE_FIXTURE: &str =
    include_str!("../../../api/examples/kernel/v1beta1/kmp/trace.response.json");
const INSPECT_RESPONSE_FIXTURE: &str =
    include_str!("../../../api/examples/kernel/v1beta1/kmp/inspect.response.json");

#[derive(Default)]
pub struct KernelMcpServer;

impl KernelMcpServer {
    pub fn handle_json_line(&self, line: &str) -> Option<String> {
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
            Some("initialize") => id.map(|id| jsonrpc_result(id, initialize_result())),
            Some("notifications/initialized") => None,
            Some("tools/list") => id.map(|id| jsonrpc_result(id, tools_list_result())),
            Some("tools/call") => id.map(|id| self.handle_tool_call(id, request.get("params"))),
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

    fn handle_tool_call(&self, id: Value, params: Option<&Value>) -> String {
        let Some(params) = params.and_then(Value::as_object) else {
            return jsonrpc_error(id, -32602, "tools/call requires object params");
        };
        let Some(name) = params.get("name").and_then(Value::as_str) else {
            return jsonrpc_error(id, -32602, "tools/call requires params.name");
        };
        let arguments = params.get("arguments").unwrap_or(&Value::Null);

        let result = match name {
            "kernel_wake" => read_fixture_tool_result(arguments, &["about"], WAKE_RESPONSE_FIXTURE),
            "kernel_ask" => {
                read_fixture_tool_result(arguments, &["about", "question"], ASK_RESPONSE_FIXTURE)
            }
            "kernel_trace" => {
                read_fixture_tool_result(arguments, &["from", "to"], TRACE_RESPONSE_FIXTURE)
            }
            "kernel_inspect" => {
                read_fixture_tool_result(arguments, &["ref"], INSPECT_RESPONSE_FIXTURE)
            }
            "kernel_remember" => {
                Err("kernel_remember is not implemented in the read-only MCP adapter".to_string())
            }
            other => Err(format!("unknown KMP tool `{other}`")),
        };

        match result {
            Ok(result) => jsonrpc_result(id, result),
            Err(message) => jsonrpc_result(id, tool_error_result(&message)),
        }
    }
}

fn initialize_result() -> Value {
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": {
            "tools": {}
        },
        "serverInfo": {
            "name": SERVER_NAME,
            "version": SERVER_VERSION
        }
    })
}

fn tools_list_result() -> Value {
    json!({
        "tools": [
            tool_definition(
                "kernel_wake",
                "Return a compact Kernel Memory Protocol wake packet for continuing work from memory.",
                json!({
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["about"],
                    "properties": {
                        "about": string_schema("Memory anchor or root ref to wake from."),
                        "role": string_schema("Optional caller role."),
                        "intent": string_schema("Optional continuation intent."),
                        "budget": budget_schema()
                    }
                })
            ),
            tool_definition(
                "kernel_ask",
                "Answer a question from kernel memory with proof, or return unknown/conflict.",
                json!({
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["about", "question"],
                    "properties": {
                        "about": string_schema("Memory anchor or root ref to ask from."),
                        "question": string_schema("Natural-language question."),
                        "answer_policy": {
                            "type": "string",
                            "enum": ["evidence_or_unknown", "show_conflicts", "best_effort"]
                        },
                        "prefer": {
                            "type": "object",
                            "additionalProperties": true
                        },
                        "budget": budget_schema()
                    }
                })
            ),
            tool_definition(
                "kernel_trace",
                "Trace the proof path between two memory refs.",
                json!({
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["from", "to"],
                    "properties": {
                        "from": string_schema("Source memory ref."),
                        "to": string_schema("Target memory ref."),
                        "goal": string_schema("Optional trace goal."),
                        "include": {
                            "type": "object",
                            "additionalProperties": false,
                            "properties": {
                                "evidence": {"type": "boolean"},
                                "raw_refs": {"type": "boolean"}
                            }
                        }
                    }
                })
            ),
            tool_definition(
                "kernel_inspect",
                "Inspect the raw stored memory object, links, and evidence for one ref.",
                json!({
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["ref"],
                    "properties": {
                        "ref": string_schema("Memory ref to inspect."),
                        "include": {
                            "type": "object",
                            "additionalProperties": false,
                            "properties": {
                                "incoming": {"type": "boolean"},
                                "outgoing": {"type": "boolean"},
                                "details": {"type": "boolean"},
                                "raw": {"type": "boolean"}
                            }
                        }
                    }
                })
            )
        ]
    })
}

fn tool_definition(name: &str, description: &str, input_schema: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema
    })
}

fn string_schema(description: &str) -> Value {
    json!({
        "type": "string",
        "minLength": 1,
        "description": description
    })
}

fn budget_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "tokens": {
                "type": "integer",
                "minimum": 1
            },
            "detail": {
                "type": "string",
                "enum": ["compact", "balanced", "full"]
            }
        }
    })
}

fn read_fixture_tool_result(
    arguments: &Value,
    required_arguments: &[&str],
    fixture: &str,
) -> Result<Value, String> {
    let Some(arguments) = arguments.as_object() else {
        return Err("tool arguments must be a JSON object".to_string());
    };

    for required_argument in required_arguments {
        let present = arguments
            .get(*required_argument)
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty());

        if !present {
            return Err(format!("missing required argument `{required_argument}`"));
        }
    }

    let structured_content = serde_json::from_str::<Value>(fixture)
        .map_err(|error| format!("fixture response is invalid JSON: {error}"))?;
    Ok(tool_success_result(structured_content))
}

fn tool_success_result(structured_content: Value) -> Value {
    let text = serde_json::to_string_pretty(&structured_content)
        .expect("fixture JSON should serialize as pretty text");
    json!({
        "content": [
            {
                "type": "text",
                "text": text
            }
        ],
        "structuredContent": structured_content,
        "isError": false
    })
}

fn tool_error_result(message: &str) -> Value {
    json!({
        "content": [
            {
                "type": "text",
                "text": message
            }
        ],
        "isError": true
    })
}

fn jsonrpc_result(id: Value, result: Value) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
    .to_string()
}

fn jsonrpc_error(id: Value, code: i64, message: &str) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    })
    .to_string()
}
