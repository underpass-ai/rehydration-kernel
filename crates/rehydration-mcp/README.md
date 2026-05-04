# Rehydration MCP

Draft stdio MCP adapter for Kernel Memory Protocol (KMP).

Current status:

- exposes `kernel_ingest`, `kernel_wake`, `kernel_ask`, `kernel_goto`,
  `kernel_near`, `kernel_rewind`, `kernel_forward`, `kernel_trace`, and
  `kernel_inspect`;
- can serve explicit fixture-backed KMP responses from
  `api/examples/kernel/v1beta1/kmp`;
- can use the live gRPC kernel when `REHYDRATION_KERNEL_GRPC_ENDPOINT` is set;
- current live mode still maps KMP tools to the lower-level
  `ContextCommandService` and `ContextQueryService` clients;
- migration to the typed `KernelMemoryService` is the next slice now that the
  gRPC memory API exists;
- live `kernel_ask` returns evidence/proof from `GetContext`, not a generated
  answer.

Run locally:

```bash
REHYDRATION_MCP_BACKEND=fixture cargo run -p rehydration-mcp --locked
```

Install from Git:

```bash
cargo install --git https://github.com/underpass-ai/rehydration-kernel rehydration-mcp --locked
```

Install with the repo helper:

```bash
bash scripts/mcp/install-rehydration-mcp.sh
```

Live gRPC backend:

```bash
REHYDRATION_KERNEL_GRPC_ENDPOINT=http://127.0.0.1:50051 cargo run -p rehydration-mcp --locked
```

Public HTTPS endpoint:

```bash
REHYDRATION_KERNEL_GRPC_ENDPOINT=https://rehydration-kernel.underpassai.com cargo run -p rehydration-mcp --locked
```

The server reads newline-delimited JSON-RPC requests from stdin and writes
newline-delimited JSON-RPC responses to stdout.
The executable is fail-fast by default: set `REHYDRATION_KERNEL_GRPC_ENDPOINT`
for live gRPC mode, or set `REHYDRATION_MCP_BACKEND=fixture` explicitly for
fixture mode.

Minimal smoke request:

```json
{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}
```

Repo smoke script:

```bash
REHYDRATION_MCP_BACKEND=fixture REHYDRATION_MCP_BIN=rehydration-mcp \
  bash scripts/mcp/kmp-stdio-smoke.sh
```

Smoke an installed binary:

```bash
REHYDRATION_KERNEL_GRPC_ENDPOINT=https://rehydration-kernel.underpassai.com \
REHYDRATION_MCP_BIN=rehydration-mcp \
  bash scripts/mcp/kmp-stdio-smoke.sh
```

Tool call example:

```json
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"kernel_ask","arguments":{"about":"question:830ce83f","question":"Where did Rachel move after her recent relocation?","answer_policy":"evidence_or_unknown"}}}
```

Current live backend mapping:

| Tool | Kernel read |
|:-----|:------------|
| `kernel_ingest` | `UpdateContext` |
| `kernel_wake` | `GetContext` |
| `kernel_ask` | `GetContext` |
| `kernel_goto` | `GetContext` |
| `kernel_near` | `GetContext` |
| `kernel_rewind` | `GetContext` |
| `kernel_forward` | `GetContext` |
| `kernel_trace` | `GetContextPath` |
| `kernel_inspect` | `GetNodeDetail` |

Next live backend mapping:

| Tool | Kernel read/write |
|:-----|:------------------|
| `kernel_ingest` | `KernelMemoryService.Ingest` |
| `kernel_wake` | `KernelMemoryService.Wake` |
| `kernel_ask` | `KernelMemoryService.Ask` |
| `kernel_goto` | `KernelMemoryService.Goto` |
| `kernel_near` | `KernelMemoryService.Near` |
| `kernel_rewind` | `KernelMemoryService.Rewind` |
| `kernel_forward` | `KernelMemoryService.Forward` |
| `kernel_trace` | `KernelMemoryService.Trace` |
| `kernel_inspect` | `KernelMemoryService.Inspect` |
