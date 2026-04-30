# Rehydration MCP

Draft stdio MCP adapter for Kernel Memory Protocol (KMP).

Current status:

- exposes read-only tools: `kernel_wake`, `kernel_ask`, `kernel_trace`,
  `kernel_inspect`;
- returns fixture-backed KMP responses from
  `api/examples/kernel/v1beta1/kmp`;
- does not implement `kernel_remember` yet;
- does not call the live gRPC kernel yet.

Run locally:

```bash
cargo run -p rehydration-mcp
```

The server reads newline-delimited JSON-RPC requests from stdin and writes
newline-delimited JSON-RPC responses to stdout.

Minimal smoke request:

```json
{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}
```

Tool call example:

```json
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"kernel_ask","arguments":{"about":"question:830ce83f","question":"Where did Rachel move after her recent relocation?","answer_policy":"evidence_or_unknown"}}}
```
