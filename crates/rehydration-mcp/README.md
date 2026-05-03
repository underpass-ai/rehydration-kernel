# Rehydration MCP

Draft stdio MCP adapter for Kernel Memory Protocol (KMP).

Current status:

- exposes `kernel_ingest`, `kernel_wake`, `kernel_ask`, `kernel_trace`, and
  `kernel_inspect`;
- defaults to fixture-backed KMP responses from `api/examples/kernel/v1beta1/kmp`;
- can use the live gRPC kernel when `REHYDRATION_KERNEL_GRPC_ENDPOINT` is set;
- maps live `kernel_ingest` to `ContextCommandService.UpdateContext`;
- live `kernel_ask` returns evidence/proof from `GetContext`, not a generated
  answer.

Run locally:

```bash
cargo run -p rehydration-mcp
```

Live gRPC backend:

```bash
REHYDRATION_KERNEL_GRPC_ENDPOINT=http://127.0.0.1:50051 cargo run -p rehydration-mcp
```

The server reads newline-delimited JSON-RPC requests from stdin and writes
newline-delimited JSON-RPC responses to stdout.

Minimal smoke request:

```json
{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}
```

Repo smoke script:

```bash
bash scripts/mcp/kmp-stdio-smoke.sh
```

Tool call example:

```json
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"kernel_ask","arguments":{"about":"question:830ce83f","question":"Where did Rachel move after her recent relocation?","answer_policy":"evidence_or_unknown"}}}
```

Live backend mapping:

| Tool | Kernel read |
|:-----|:------------|
| `kernel_ingest` | `UpdateContext` |
| `kernel_wake` | `GetContext` |
| `kernel_ask` | `GetContext` |
| `kernel_trace` | `GetContextPath` |
| `kernel_inspect` | `GetNodeDetail` |
