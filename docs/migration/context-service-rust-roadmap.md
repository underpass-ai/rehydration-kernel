# Context Service Rust Roadmap

Status: Active
Source of truth: [`CONTEXT_SERVICE_RUST_MIGRATION_PLAN.md`](../../CONTEXT_SERVICE_RUST_MIGRATION_PLAN.md)

## Purpose

Turn the migration RFC into an execution roadmap for this repo.

This roadmap keeps two rules explicit:

1. the internal core stays node-centric
2. compatibility with the existing Context Service is implemented at the edge

External legacy nouns appear in this roadmap only when naming the frozen
boundary contract.

## Non-Negotiable Constraints

- no god objects
- no god files
- DDD first
- hexagonal boundaries
- one main concept per file
- one use case per file
- one command or query entry point per file
- legacy compatibility only at adapters and transport edges
- the core language remains:
  - root node
  - neighbor nodes
  - relationships
  - extended node detail in Valkey

## Current Baseline

`main` already provides the substrate needed before starting compatibility work:

- graph-native bundle domain
- focused application use cases
- split Neo4j adapter
- split Valkey adapter
- split NATS adapter
- split gRPC transport
- green repo quality gates

That means the next milestone is not another core refactor.
The next milestone is the compatibility shell.

## Status by Stream

### Stream A: Internal node-centric core

Status: `done for foundation`

Delivered:

- graph-native domain
- focused application layer
- modular adapters
- modular transport
- quality gates and tests

This stream should reopen only if a compatibility slice exposes a real core
gap.

### Stream B: External contract compatibility

Status: `phase 0 complete, phase 1 next`

Frozen in Phase 0:

- external gRPC inventory
- external NATS subjects
- EventEnvelope rules
- config and startup behavior
- compatibility matrix
- golden test catalog
- reuse boundary

### Stream C: Rollout and shadow mode

Status: `not started`

Missing:

- dual-run strategy
- shadow comparison harness
- canary and rollback procedure

## Phase Roadmap

### Phase 0: Contract Freeze

Status: `complete`

Goal:

- freeze the observable contract of the existing Context Service before further
  implementation work

Outputs:

- [`context-service-phase0-contract-freeze.md`](./context-service-phase0-contract-freeze.md)
- [`context-service-compatibility-matrix.md`](./context-service-compatibility-matrix.md)
- [`context-service-golden-tests.md`](./context-service-golden-tests.md)
- [`rehydration-kernel-reuse-boundary.md`](./rehydration-kernel-reuse-boundary.md)

Exit gate: complete

### Phase 1: Compatibility Shell

Status: `next`

Goal:

- expose `fleet.context.v1` in Rust without leaking external legacy nouns into
  the node-centric core

Deliverables:

- compatibility proto package and transport module
- one `ContextService` boundary facade
- focused request DTO files
- focused response DTO files
- explicit request mappers
- explicit response mappers
- compatibility status and error mapping
- compatibility bootstrap for external config defaults

Structural rules:

- separate files for DTOs, request mapping, response mapping, and status mapping
- no transport god service
- no compatibility mapper file that owns unrelated RPCs

Exit gate:

- the Rust service boots with the external package and service identity
- boundary error mapping is proven by tests
- the shell routes requests into the current node-centric application layer

### Phase 2: Read-Path Parity

Status: `pending`

Goal:

- serve the external read API from the node-centric core

Scope:

- `GetContext`
- `RehydrateSession`
- `ValidateScope`
- `GetGraphRelationships`

Deliverables:

- request mapping from external identifiers into node-centric workflows
- response rendering from graph-native outputs into external DTOs
- depth clamp and validation parity for `GetGraphRelationships`
- scope validation parity
- golden tests for all read RPCs

Exit gate:

- read-path golden tests pass
- external DTOs match the frozen contract
- no core module adopts external legacy nouns

### Phase 3: Async NATS Parity

Status: `pending`

Goal:

- implement the async request or reply and event flows required by the existing
  service

Scope:

- `context.update.request`
- `context.rehydrate.request`
- `context.update.response`
- `context.rehydrate.response`
- `context.events.updated`

Deliverables:

- subject routers per public flow
- EventEnvelope parsing and validation
- request or reply correlation
- `ack` or `nak` parity
- publish envelope parity

Exit gate:

- async golden tests pass
- envelope behavior matches the baseline
- public subjects are implemented without turning the NATS adapter into a god file

### Phase 4: Write-Path Parity

Status: `pending`

Goal:

- support the external write API while keeping the core node-centric

Scope:

- `UpdateContext`
- `CreateStory`
- `CreateTask`
- `AddProjectDecision`
- `TransitionPhase`

Deliverables:

- focused compatibility commands and mappers
- write-path response parity
- response publication parity where applicable
- golden tests for all write RPCs

Exit gate:

- write-path golden tests pass
- event publication parity is verified
- no external write DTO leaks into the domain core

### Phase 5: Rollout

Status: `pending`

Goal:

- cut over safely from the Python service to the Rust implementation

Deliverables:

- shadow comparison harness
- canary procedure
- rollback procedure
- cutover checklist

Exit gate:

- shadow comparisons are within accepted drift
- rollback is exercised
- production cutover checklist is signed off

## Immediate Next Slice

Start Phase 1 with the compatibility shell, not with more internal refactor.

The first implementation cut should produce:

1. external package and service scaffolding for `fleet.context.v1`
2. focused edge DTO modules
3. error mapping parity at the boundary
4. tests that prove the compatibility shell boots without polluting the core
