# Kernel Runtime Integration Reference

Status: Active
Scope: reference contract for external runtimes that consume the kernel

## Intent

Document the smallest practical integration shape for an agent runtime that
uses `rehydration-kernel` as its context engine.

This is not a kernel-owned transport contract in the same sense as:

- `underpass.rehydration.kernel.v1alpha1`
- `graph.node.materialized`
- `node.detail.materialized`
- `context.bundle.generated`

Those are kernel contracts.

This document instead defines a recommended consumer-side runtime shape that is
proven by the agentic end-to-end test and is suitable for:

- a sibling runtime such as `underpass-runtime`
- a custom coding-agent runtime
- a research or orchestration runtime
- any process that exposes tool discovery and tool invocation to an agent

## Design Rules

The runtime integration must keep the kernel generic.

That means:

- the runtime sends `root_node_id`, not product-specific backlog ids
- the runtime discovers focus through graph relationships
- the runtime consumes rendered context and node detail as generic data
- no runtime-specific nouns are added to kernel domain or transport contracts

## Minimum Runtime Capabilities

The current reference agent needs only three runtime capabilities:

- `fs.write`
- `fs.read`
- `fs.list`

Those are not special kernel requirements. They are just the minimal tools used
by the end-to-end proof.

A different runtime may expose:

- editor tools
- shell tools
- browser tools
- planner tools

The kernel only assumes that an agent can:

1. discover available tools
2. invoke a chosen tool
3. receive the invocation result

## Reference Runtime HTTP Shape

The reference runtime shape used by the agentic E2E is:

1. `POST /v1/sessions`
2. `GET /v1/sessions/{session_id}/tools`
3. `POST /v1/sessions/{session_id}/tools/{tool_name}/invoke`

The shared client and fake runtime that prove this shape are:

- [`runtime_http_client.rs`](../../crates/rehydration-transport-grpc/src/agentic_reference/runtime_http_client.rs)
- [`fake_underpass_runtime.rs`](../../crates/rehydration-transport-grpc/tests/support/fake_underpass_runtime.rs)
- [`main.rs`](../../crates/rehydration-transport-grpc/src/bin/runtime_reference_client/main.rs)

This shape is intentionally narrow:

- session creation gives the agent a runtime session id
- tool discovery tells the agent what actions are available
- tool invocation executes a named tool with JSON args

## Reference Payload Shape

Reference examples live under:

- [`api/examples/runtime-reference/v1/README.md`](../../api/examples/runtime-reference/v1/README.md)

The core payloads are:

- create session request or response
- list tools response
- invoke tool request or response

Each tool entry carries:

- `name`
- `requires_approval`

Each invocation request carries:

- `args`
- `approved`

Each invocation response carries:

- `tool_name`
- `output`

## Reference Client Outside Tests

The repository also contains a runnable reference client outside `tests/`:

- [`main.rs`](../../crates/rehydration-transport-grpc/src/bin/runtime_reference_client/main.rs)

It is configured with environment variables:

- `KERNEL_GRPC_ENDPOINT`
- `RUNTIME_BASE_URL`
- `ROOT_NODE_ID`
- optional `ROOT_NODE_KIND`
- optional `AGENT_ROLE`
- optional `AGENT_PHASE`
- optional `AGENT_FOCUS_NODE_KIND`
- optional `AGENT_SCOPES`
- optional `AGENT_TOKEN_BUDGET`
- optional `AGENT_SUMMARY_PATH`

The binary prints a JSON execution summary so external runtimes can copy the
flow without depending on the test harness.

## Kernel Call Sequence

The reference agent flow is implemented in:

- [`basic_context_agent.rs`](../../crates/rehydration-transport-grpc/src/agentic_reference/basic_context_agent.rs)
- [`config.rs`](../../crates/rehydration-transport-grpc/src/bin/runtime_reference_client/config.rs)

The sequence is:

1. list runtime tools
2. validate that required tools exist
3. call `GetGraphRelationships`
4. select a focus node from neighbors
5. call `GetContext`
6. build a runtime artifact from:
   - root node title
   - focused node title
   - focused node detail
   - rendered context
7. invoke runtime tools

## Recommended Kernel Usage Pattern

### 1. Select a root node

The runtime or upstream agent should start from:

- a workspace node
- a task-like node
- a claim node
- an incident node
- any other product-defined node kind

The runtime does not need the kernel to know what those mean.

### 2. Find local focus

Call `GetGraphRelationships` first when the runtime needs to narrow focus.

This avoids hardcoding a product taxonomy into the kernel.

### 3. Fetch bounded context

Call `GetContext` with:

- `root_node_id`
- role
- optional focused node id
- optional scopes
- token budget

The runtime should treat the rendered payload as bounded working context, not
as the full source of truth.

### 4. Use node detail when needed

When a focused node has extended detail, the runtime should prefer that detail
for precise tool execution, while using rendered context for broader grounding.

## Ownership Boundary

The runtime reference shape is not a kernel-owned protocol commitment.

That means:

- the kernel does not expose these HTTP paths
- the kernel does not guarantee every external runtime will use this shape
- the examples here are guidance for external consumers

The owned kernel boundary remains:

- gRPC
- generic async subjects
- node-centric payload semantics

## What This Reference Already Proves

The current E2E proves:

- a runtime can consume node-centric kernel context without fleet-specific nouns
- an agent can derive focus from graph structure
- an agent can drive tool execution from rendered context plus node detail
- a session-tools-invoke runtime shape is enough for a basic integration
- the same agent flow can be triggered from `context.bundle.generated`

See:

- [`kernel-agentic-integration-e2e.md`](./kernel-agentic-integration-e2e.md)
- [`kernel-agentic-event-trigger-e2e.md`](./kernel-agentic-event-trigger-e2e.md)

## What It Does Not Yet Freeze

This reference does not yet freeze:

- auth headers
- tenancy propagation
- long-lived session renewal
- streaming outputs
- tool cancellation
- richer approval workflows
- a runtime-owned standard for subscribing, checkpointing, or retrying
  `context.bundle.generated`

Those should be added only when they are proven necessary by a real consumer.
