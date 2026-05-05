# MCP Stdio Adapter

Status: installable stdio adapter for Kernel Memory Protocol (KMP).

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

In live mode, the adapter calls the typed gRPC `KernelMemoryService` API. The
MCP process owns JSON-RPC parsing, MCP tool schemas, JSON/proto conversion,
structuredContent conversion, and TLS endpoint configuration. It does not call
`ContextQueryService` or `ContextCommandService` directly for KMP moves.
Successful live ingest responses report `read_after_write_ready=true`; fixture
and dry-run responses remain `read_after_write_ready=false`.

Temporal tools call `KernelMemoryService.Goto`, `Near`, `Rewind`, and
`Forward`. Temporal traversal and multidimensional scoping are kernel behavior,
not MCP-side reconstruction.

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

Live mode requires the deployed kernel to expose `KernelMemoryService`. There is
no compatibility fallback to lower-level query/command services.

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
| `kernel_ingest` | `KernelMemoryService.Ingest` |
| `kernel_wake` | `KernelMemoryService.Wake` |
| `kernel_ask` | `KernelMemoryService.Ask` |
| `kernel_goto` | `KernelMemoryService.Goto` |
| `kernel_near` | `KernelMemoryService.Near` |
| `kernel_rewind` | `KernelMemoryService.Rewind` |
| `kernel_forward` | `KernelMemoryService.Forward` |
| `kernel_trace` | `KernelMemoryService.Trace` |
| `kernel_inspect` | `KernelMemoryService.Inspect` |

In live mode, `kernel_ask` returns a deterministic evidence-derived answer or
`UNKNOWN`; it does not generate a final LLM answer. The temporal tools return
deterministic traversal slices from kernel-owned temporal traversal.

Dimension scope is explicit and auditable:

- omitted scope defaults to `current_about`;
- `abouts` requires a non-empty `abouts` list and fails fast otherwise;
- `all_abouts` intentionally traverses every memory anchor from the kernel
  memory about index;
- `scope_ids` can use local dimension ids or fully namespaced
  `about:<about>:dimension:<dimension_id>` ids.

`kernel_inspect` supports typed object/detail/incoming/outgoing/evidence lookup.
`include.raw=true` returns typed raw audit refs for the inspected object.

Temporal `include.raw_refs=true` returns typed raw audit refs for selected
entries. `include.evidence` and `include.relations` are supported.

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
endpoint to the MCP adapter, verifies `kernel_ingest` through
`KernelMemoryService.Ingest`, verifies that the ingested memory can be read back
with `kernel_wake`, verifies `kernel_forward` through typed temporal traversal,
and verifies `kernel_wake`, `kernel_ask`, `kernel_trace`, and `kernel_inspect`
against the seeded live read model.

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
