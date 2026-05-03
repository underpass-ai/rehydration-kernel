use serde_json::Value;

use crate::args::validate_required_arguments;
use crate::backend::{KernelMcpToolBackend, KernelMcpToolFuture};
use crate::protocol::tool_success_result;

const WAKE_RESPONSE_FIXTURE: &str =
    include_str!("../../../api/examples/kernel/v1beta1/kmp/wake.response.json");
const ASK_RESPONSE_FIXTURE: &str =
    include_str!("../../../api/examples/kernel/v1beta1/kmp/ask.response.json");
const TRACE_RESPONSE_FIXTURE: &str =
    include_str!("../../../api/examples/kernel/v1beta1/kmp/trace.response.json");
const INSPECT_RESPONSE_FIXTURE: &str =
    include_str!("../../../api/examples/kernel/v1beta1/kmp/inspect.response.json");

#[derive(Clone, Copy, Debug, Default)]
pub struct FixtureKernelMcpBackend;

impl KernelMcpToolBackend for FixtureKernelMcpBackend {
    fn backend_name(&self) -> &'static str {
        "fixture"
    }

    fn call_tool<'a>(&'a self, name: &'a str, arguments: &'a Value) -> KernelMcpToolFuture<'a> {
        Box::pin(async move { fixture_tool_result(name, arguments) })
    }
}

pub(crate) fn fixture_tool_result(name: &str, arguments: &Value) -> Result<Value, String> {
    match name {
        "kernel_wake" => read_fixture_tool_result(arguments, &["about"], WAKE_RESPONSE_FIXTURE),
        "kernel_ask" => {
            read_fixture_tool_result(arguments, &["about", "question"], ASK_RESPONSE_FIXTURE)
        }
        "kernel_trace" => {
            read_fixture_tool_result(arguments, &["from", "to"], TRACE_RESPONSE_FIXTURE)
        }
        "kernel_inspect" => read_fixture_tool_result(arguments, &["ref"], INSPECT_RESPONSE_FIXTURE),
        "kernel_ingest" | "kernel_remember" => {
            Err("kernel_ingest is not implemented in the read-only MCP adapter".to_string())
        }
        other => Err(format!("unknown KMP tool `{other}`")),
    }
}

fn read_fixture_tool_result(
    arguments: &Value,
    required_arguments: &[&str],
    fixture: &str,
) -> Result<Value, String> {
    validate_required_arguments(arguments, required_arguments)?;
    let structured_content = serde_json::from_str::<Value>(fixture)
        .map_err(|error| format!("fixture response is invalid JSON: {error}"))?;
    Ok(tool_success_result(structured_content))
}
