use std::io;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::logging::{debug_log, debug_log_value};
use crate::runtime_contract::{AgentRuntime, RuntimeResult, ToolDescriptor, ToolInvocation};
use crate::starship_runtime_tools::{
    STARSHIP_LIST_TOOL, all_supported_tools, is_write_tool, path_for_tool_name,
};

#[derive(Clone)]
pub struct UnderpassRuntimeClient {
    authority: String,
    session_id: String,
}

impl UnderpassRuntimeClient {
    pub async fn connect(base_url: impl Into<String>) -> RuntimeResult<Self> {
        let base_url = base_url.into().trim_end_matches('/').to_string();
        let authority = extract_authority(&base_url)?;
        debug_log_value("runtime client connect", &base_url);
        let response: CreateSessionResponse =
            send_json_request(&authority, "POST", "/v1/sessions", &json!({})).await?;
        debug_log_value("runtime client session_id", &response.session_id);

        Ok(Self {
            authority,
            session_id: response.session_id,
        })
    }

    fn tools_path(&self) -> String {
        format!("/v1/sessions/{}/tools", self.session_id)
    }

    fn invoke_path(&self, tool_name: &str) -> String {
        format!("{}/{tool_name}/invoke", self.tools_path())
    }
}

impl AgentRuntime for UnderpassRuntimeClient {
    async fn list_tools(&self) -> RuntimeResult<Vec<ToolDescriptor>> {
        debug_log("runtime client list_tools");
        let _response: Value =
            send_json_request(&self.authority, "GET", &self.tools_path(), &json!({})).await?;

        Ok(all_supported_tools()
            .into_iter()
            .map(|name| ToolDescriptor {
                name: name.to_string(),
                requires_approval: is_write_tool(name),
            })
            .collect())
    }

    async fn invoke(
        &self,
        tool_name: &str,
        args: Value,
        approved: bool,
    ) -> RuntimeResult<ToolInvocation> {
        debug_log_value("runtime client invoke", tool_name);
        let (runtime_tool_name, runtime_args) = translate_runtime_request(tool_name, args)?;
        let response: InvokeToolResponse = send_json_request(
            &self.authority,
            "POST",
            &self.invoke_path(runtime_tool_name),
            &InvokeToolRequest {
                args: runtime_args,
                approved,
            },
        )
        .await?;

        Ok(ToolInvocation {
            tool_name: tool_name.to_string(),
            output: response.output,
        })
    }
}

#[derive(Debug, Deserialize)]
struct CreateSessionResponse {
    session_id: String,
}

#[derive(Debug, Serialize)]
struct InvokeToolRequest {
    args: Value,
    approved: bool,
}

#[derive(Debug, Deserialize)]
struct InvokeToolResponse {
    output: String,
}

fn translate_runtime_request(tool_name: &str, args: Value) -> RuntimeResult<(&'static str, Value)> {
    if tool_name == STARSHIP_LIST_TOOL {
        return Ok(("fs.list", json!({})));
    }

    if let Some(path) = path_for_tool_name(tool_name) {
        if is_write_tool(tool_name) {
            let content = args.get("content").and_then(Value::as_str).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "missing string arg `content`")
            })?;
            return Ok(("fs.write", json!({ "path": path, "content": content })));
        }

        return Ok(("fs.read", json!({ "path": path })));
    }

    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        format!("unsupported runtime tool `{tool_name}`"),
    )
    .into())
}

fn extract_authority(base_url: &str) -> io::Result<String> {
    base_url
        .strip_prefix("http://")
        .map(ToString::to_string)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "unsupported runtime base url"))
}

async fn send_json_request<T, B>(
    authority: &str,
    method: &str,
    path: &str,
    body: &B,
) -> RuntimeResult<T>
where
    T: for<'de> Deserialize<'de>,
    B: Serialize,
{
    let mut stream = TcpStream::connect(authority).await?;
    let body = serde_json::to_vec(body)?;
    debug_log_value("runtime http request", format!("{method} {path}"));
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: {authority}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );

    stream.write_all(request.as_bytes()).await?;
    if !body.is_empty() {
        stream.write_all(&body).await?;
    }
    stream.shutdown().await?;

    let mut response = Vec::new();
    stream.read_to_end(&mut response).await?;
    let response = String::from_utf8(response)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    debug_log_value("runtime http raw response bytes", response.len());

    parse_json_response(&response)
}

fn parse_json_response<T>(response: &str) -> RuntimeResult<T>
where
    T: for<'de> Deserialize<'de>,
{
    let (headers, body) = response.split_once("\r\n\r\n").ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "runtime response missing body separator",
        )
    })?;
    let status_line = headers.lines().next().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "runtime response missing status line",
        )
    })?;
    let status_code = status_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "runtime status missing"))?;

    if !status_code.starts_with('2') {
        return Err(io::Error::other(format!("runtime request failed: {status_line}")).into());
    }

    Ok(serde_json::from_str(body)?)
}
