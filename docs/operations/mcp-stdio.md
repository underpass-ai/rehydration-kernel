# MCP Stdio Adapter

Status: draft local adapter for Kernel Memory Protocol (KMP).

The repo includes a stdio MCP server in
[`crates/rehydration-mcp`](../../crates/rehydration-mcp). It exposes the
read-only KMP tools:

- `kernel_wake`
- `kernel_ask`
- `kernel_trace`
- `kernel_inspect`

`kernel_remember` is deliberately not implemented yet.

## Modes

Default mode uses local fixtures. This is useful for client wiring, demos, and
tool-choice validation without running Neo4j, Valkey, NATS, or the kernel
server.

```bash
cargo run -p rehydration-mcp --locked
```

Live mode uses the existing gRPC `ContextQueryService`.

```bash
REHYDRATION_KERNEL_GRPC_ENDPOINT=http://127.0.0.1:50051 \
  cargo run -p rehydration-mcp --locked
```

HTTPS endpoints automatically enable server TLS and use system/webpki roots:

```bash
REHYDRATION_KERNEL_GRPC_ENDPOINT=https://rehydration-kernel.underpassai.com \
  cargo run -p rehydration-mcp --locked
```

Private CAs and direct mTLS are explicit:

```bash
REHYDRATION_KERNEL_GRPC_ENDPOINT=https://rehydration-kernel.underpass-runtime.svc:50054 \
REHYDRATION_KERNEL_GRPC_TLS_MODE=mutual \
REHYDRATION_KERNEL_GRPC_TLS_CA_PATH=/var/run/kernel-tls/ca.crt \
REHYDRATION_KERNEL_GRPC_TLS_CERT_PATH=/var/run/kernel-tls/tls.crt \
REHYDRATION_KERNEL_GRPC_TLS_KEY_PATH=/var/run/kernel-tls/tls.key \
REHYDRATION_KERNEL_GRPC_TLS_DOMAIN_NAME=rehydration-kernel-grpc \
  cargo run -p rehydration-mcp --locked
```

Live mapping:

| MCP tool | Kernel read |
|:---------|:------------|
| `kernel_wake` | `GetContext` |
| `kernel_ask` | `GetContext` |
| `kernel_trace` | `GetContextPath` |
| `kernel_inspect` | `GetNodeDetail` |

In live mode, `kernel_ask` returns evidence and proof from `GetContext`; it
does not generate a final answer yet.

## Smoke Test

Fixture mode:

```bash
bash scripts/mcp/kmp-stdio-smoke.sh
```

Live mode:

```bash
REHYDRATION_KERNEL_GRPC_ENDPOINT=http://127.0.0.1:50051 \
KMP_MCP_SMOKE_REF=node:mission:engine-core-failure \
  bash scripts/mcp/kmp-stdio-smoke.sh
```

`KMP_MCP_SMOKE_REF` must be a node id that exists in the live kernel read model.

Real kernel integration smoke:

```bash
bash scripts/ci/integration-mcp-real-kernel.sh
```

This starts the containerized Kernel test fixture, exposes its ephemeral gRPC
endpoint to the MCP adapter, and verifies `kernel_wake`, `kernel_ask`,
`kernel_trace`, and `kernel_inspect` against the live read model.

## Generic MCP Client Config

Use the repo root as the working directory and start the server with Cargo:

```toml
[mcp_servers.rehydration-kernel]
command = "cargo"
args = ["run", "-q", "-p", "rehydration-mcp", "--locked"]
```

Live gRPC mode:

```toml
[mcp_servers.rehydration-kernel]
command = "cargo"
args = ["run", "-q", "-p", "rehydration-mcp", "--locked"]
env = { REHYDRATION_KERNEL_GRPC_ENDPOINT = "http://127.0.0.1:50051" }
```

If the client supports a per-server working directory, set it to the repository
root so Cargo can resolve the workspace.

## Manual JSON-RPC Check

```bash
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' \
  | cargo run -q -p rehydration-mcp --locked
```

Expected behavior:

- the server writes one JSON-RPC response per input line;
- fixture mode returns deterministic KMP fixture responses;
- live mode returns MCP tool errors instead of crashing if the gRPC endpoint is
  unavailable.
