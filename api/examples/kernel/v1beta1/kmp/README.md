# Kernel Memory Protocol Fixtures

Status: draft contract fixtures for the Kernel 1.0 public memory slice.

This folder defines the transport-neutral Kernel Memory Protocol (KMP) shape.
MCP, gRPC, and NATS should carry these same memory moves instead of inventing
separate product models per transport.

Required moves:

| Move | Request fixture | Response fixture |
|:-----|:----------------|:-----------------|
| `ingest` | `ingest.request.json` | `ingest.response.json` |
| `wake` | `wake.request.json` | `wake.response.json` |
| `ask` | `ask.request.json` | `ask.response.json` |
| `goto` | `goto.request.json` | `goto.response.json` |
| `near` | `near.request.json` | `near.response.json` |
| `rewind` | `rewind.request.json` | `rewind.response.json` |
| `forward` | `forward.request.json` | `forward.response.json` |
| `trace` | `trace.request.json` | `trace.response.json` |
| `inspect` | `inspect.request.json` | `inspect.response.json` |

Schema:

- `kernel-memory-protocol.schema.json`

The protocol contract is intentionally framed as memory moves:

```text
ingest -> wake -> ask -> goto/near/rewind/forward -> trace -> inspect
```

Transport bindings:

- MCP tools should expose `kernel_ingest`, `kernel_wake`, `kernel_ask`,
  `kernel_goto`, `kernel_near`, `kernel_rewind`, `kernel_forward`,
  `kernel_trace`, and `kernel_inspect`.
- gRPC should expose the same moves through a typed `KernelMemoryService`.
- NATS should start with asynchronous `kernel.memory.ingest`,
  `kernel.memory.ingested`, and `kernel.memory.rejected`.

These fixtures are not a replacement for the existing node-centric gRPC and
AsyncAPI contracts. They define the higher-level public memory shape that maps
onto those existing Kernel 1.0 primitives.
