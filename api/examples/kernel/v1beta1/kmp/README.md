# Kernel Memory Protocol Fixtures

Status: draft contract fixtures for the Kernel 1.0 public memory slice.

This folder defines the transport-neutral Kernel Memory Protocol (KMP) shape.
MCP, gRPC, and NATS should carry these same memory moves instead of inventing
separate product models per transport.

Required moves:

| Move | Request fixture | Response fixture |
|:-----|:----------------|:-----------------|
| `remember` | `remember.request.json` | `remember.response.json` |
| `wake` | `wake.request.json` | `wake.response.json` |
| `ask` | `ask.request.json` | `ask.response.json` |
| `trace` | `trace.request.json` | `trace.response.json` |
| `inspect` | `inspect.request.json` | `inspect.response.json` |

Schema:

- `kernel-memory-protocol.schema.json`

The protocol contract is intentionally framed as memory moves:

```text
remember -> wake -> ask -> trace -> inspect
```

Transport bindings:

- MCP tools should expose `kernel_remember`, `kernel_wake`, `kernel_ask`,
  `kernel_trace`, and `kernel_inspect`.
- gRPC should expose the same moves through a typed `KernelMemoryService`.
- NATS should start with asynchronous `kernel.memory.remember`,
  `kernel.memory.remembered`, and `kernel.memory.rejected`.

These fixtures are not a replacement for the existing node-centric gRPC and
AsyncAPI contracts. They define the higher-level public memory shape that maps
onto those existing Kernel 1.0 primitives.
