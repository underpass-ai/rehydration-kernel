# MCP Stdio Adapter

Status: draft local adapter for Kernel Memory Protocol (KMP).

The repo includes a stdio MCP server in
[`crates/rehydration-mcp`](../../crates/rehydration-mcp). It exposes the KMP
tools:

- `kernel_ingest`
- `kernel_wake`
- `kernel_ask`
- `kernel_goto`
- `kernel_near`
- `kernel_rewind`
- `kernel_forward`
- `kernel_trace`
- `kernel_inspect`

In live mode, `kernel_ingest` submits memory through the existing gRPC
`ContextCommandService.UpdateContext` command path. Acceptance means the kernel
accepted the command and synchronously projected basic KMP `memory_*` changes
into the read model. Successful live responses report
`read_after_write_ready=true`; fixture and dry-run responses remain
`read_after_write_ready=false`.

Temporal tools use the same live `GetContext` read path and interpret
`contains_entry` relationships as positions inside dimensions/scopes. The
position fields include `dimension`, `scope_id`, `sequence`, `rank`,
`occurred_at`, `observed_at`, `ingested_at`, `valid_from`, and `valid_until`.

## Modes

Fixture mode is explicit. It is useful for client wiring, demos, and
tool-choice validation without running Neo4j, Valkey, NATS, or the kernel
server.

```bash
REHYDRATION_MCP_BACKEND=fixture cargo run -p rehydration-mcp --locked
```

The executable is fail-fast by default. Without
`REHYDRATION_KERNEL_GRPC_ENDPOINT`, it exits unless
`REHYDRATION_MCP_BACKEND=fixture` is set explicitly.

## Installation

For users that do not need to work inside the repository, install the MCP
adapter as a Cargo binary from Git:

```bash
cargo install --git https://github.com/underpass-ai/rehydration-kernel rehydration-mcp --locked
```

The repository helper wraps the same install path and supports pinned refs:

```bash
bash scripts/mcp/install-rehydration-mcp.sh

REHYDRATION_MCP_TAG=v0.1.0 bash scripts/mcp/install-rehydration-mcp.sh
REHYDRATION_MCP_REV=<git-sha> bash scripts/mcp/install-rehydration-mcp.sh
```

After install, the MCP server command is just:

```bash
REHYDRATION_KERNEL_GRPC_ENDPOINT=https://rehydration-kernel.underpassai.com rehydration-mcp
```

The crate is not yet published to crates.io. The supported external install
path for this phase is `cargo install --git`, because the generated gRPC proto
client still builds from the repository's checked-in proto tree.

Live mode uses the existing gRPC `ContextQueryService` and
`ContextCommandService`.

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

| MCP tool | Kernel binding |
|:---------|:------------|
| `kernel_ingest` | `UpdateContext` |
| `kernel_wake` | `GetContext` |
| `kernel_ask` | `GetContext` |
| `kernel_goto` | `GetContext` |
| `kernel_near` | `GetContext` |
| `kernel_rewind` | `GetContext` |
| `kernel_forward` | `GetContext` |
| `kernel_trace` | `GetContextPath` |
| `kernel_inspect` | `GetNodeDetail` |

In live mode, `kernel_ask` returns evidence and proof from `GetContext`; it
does not generate a final answer yet.
The temporal tools return deterministic traversal slices from graph positions;
they do not synthesize a generated final answer.

## Smoke Test

Fixture mode:

```bash
REHYDRATION_MCP_BACKEND=fixture \
REHYDRATION_MCP_BIN=rehydration-mcp \
  bash scripts/mcp/kmp-stdio-smoke.sh
```

Live mode:

```bash
REHYDRATION_KERNEL_GRPC_ENDPOINT=http://127.0.0.1:50051 \
REHYDRATION_MCP_BIN=rehydration-mcp \
KMP_MCP_SMOKE_REF=node:mission:engine-core-failure \
  bash scripts/mcp/kmp-stdio-smoke.sh
```

`KMP_MCP_SMOKE_REF` must be a node id that exists in the live kernel read model.

Real kernel integration smoke:

```bash
bash scripts/ci/integration-mcp-real-kernel.sh
```

This starts the containerized Kernel test fixture, exposes its ephemeral gRPC
endpoint to the MCP adapter, verifies `kernel_ingest` against the live command
service, verifies that the ingested memory can be read back with `kernel_wake`,
verifies `kernel_forward` against live `contains_entry` positions, and verifies
`kernel_wake`, `kernel_ask`, `kernel_trace`, and `kernel_inspect` against the
seeded live read model.

## Generic MCP Client Config

For installed usage, point the MCP client directly at the binary:

```toml
[mcp_servers.rehydration-kernel]
command = "rehydration-mcp"
```

Live gRPC mode:

```toml
[mcp_servers.rehydration-kernel]
command = "rehydration-mcp"
env = { REHYDRATION_KERNEL_GRPC_ENDPOINT = "https://rehydration-kernel.underpassai.com" }
```

For development from a checkout, use the repo root as the working directory and
start the server with Cargo:

```toml
[mcp_servers.rehydration-kernel]
command = "cargo"
args = ["run", "-q", "-p", "rehydration-mcp", "--locked"]
env = { REHYDRATION_KERNEL_GRPC_ENDPOINT = "https://rehydration-kernel.underpassai.com" }
```

Codex CLI global config can use an absolute manifest path so it works from any
working directory:

```bash
codex mcp add rehydration-kernel \
  --env REHYDRATION_KERNEL_GRPC_ENDPOINT=https://rehydration-kernel.underpassai.com \
  -- cargo run -q --manifest-path /path/to/rehydration-kernel/Cargo.toml -p rehydration-mcp --locked
```

That writes:

```toml
[mcp_servers.rehydration-kernel]
command = "cargo"
args = ["run", "-q", "--manifest-path", "/path/to/rehydration-kernel/Cargo.toml", "-p", "rehydration-mcp", "--locked"]

[mcp_servers.rehydration-kernel.env]
REHYDRATION_KERNEL_GRPC_ENDPOINT = "https://rehydration-kernel.underpassai.com"
```

With an installed binary, the global Codex config is simpler:

```toml
[mcp_servers.rehydration-kernel]
command = "rehydration-mcp"

[mcp_servers.rehydration-kernel.env]
REHYDRATION_KERNEL_GRPC_ENDPOINT = "https://rehydration-kernel.underpassai.com"
```

When the public endpoint is served through the configured Ingress, the MCP
client uses server TLS to the public host. The Ingress handles upstream GRPCS
and the kernel backend's mutual TLS using its configured proxy client secret.

## Manual JSON-RPC Check

```bash
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' \
  | REHYDRATION_MCP_BACKEND=fixture cargo run -q -p rehydration-mcp --locked
```

Expected behavior:

- the server writes one JSON-RPC response per input line;
- fixture mode returns deterministic KMP fixture responses only when explicitly
  selected;
- live mode returns MCP tool errors instead of crashing if the gRPC endpoint is
  unavailable.
