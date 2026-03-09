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
- external `fleet.context.v1` proto package
- compatibility shell at the gRPC edge
- explicit request, response, and status mapping modules
- container-backed compatibility integration tests
- green repo quality gates

That means the next milestone is not another core refactor.
The next milestone is async parity at the external boundary.

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

Status: `phase 0 complete, phase 1 complete, phase 2 complete`

Frozen in Phase 0:

- external gRPC inventory
- external NATS subjects
- EventEnvelope rules
- config and startup behavior
- compatibility matrix
- golden test catalog
- reuse boundary

Delivered in Phase 1:

- compatibility proto generation for `fleet.context.v1`
- compatibility transport facade
- focused RPC modules
- focused request mapping modules
- focused response mapping modules
- boundary status mapping
- container-backed integration coverage for the implemented compatibility flows

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

Status: `complete`

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

Delivered:

- `fleet.context.v1` proto package and generation flow
- compatibility gRPC facade
- focused request mapping modules
- focused response mapping modules
- status mapping for implemented and placeholder RPCs
- transport tests for compatibility routing
- container-backed compatibility integration tests for:
  - `GetContext`
  - `GetGraphRelationships`
  - `RehydrateSession`

### Phase 2: Read-Path Parity

Status: `complete`

Goal:

- serve the external read API from the node-centric core

Scope:

- `GetContext`
- `RehydrateSession`
- `ValidateScope`
- `GetGraphRelationships`

Current implementation state:

- `GetContext`: routed and covered
- `RehydrateSession`: routed and covered
- `GetGraphRelationships`: routed and covered
- `ValidateScope`: routed and covered
- read-path golden tests: implemented
- DTO parity audit: completed
- `GetContext.phase`: mapped to compatibility scope expectations
- `GetContext.subtask_id`: mapped to node-centric focus rendering
- `GetContext.token_budget`: enforced as render budget hint
- `RehydrateSession.ttl_seconds`: defaulted and propagated to snapshot persistence

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

Status: `in progress`

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

Current implementation state:

- `context.update.request`: compatibility consumer implemented
- `context.rehydrate.request`: compatibility consumer implemented
- `context.update.response`: envelope publication implemented
- `context.rehydrate.response`: envelope publication implemented
- `context.events.updated`: publisher and frozen envelope publication implemented
- compatibility JetStream runtime wiring implemented for request consumers and publication sink
- compatibility NATS config implemented with `NATS_URL` default and `ENABLE_NATS` fail-fast behavior
- required `EventEnvelope` parsing and validation implemented at the NATS edge
- golden tests cover:
  - valid request -> publish reply + `ack`
  - invalid JSON -> `ack` and drop
  - invalid envelope -> `ack` and drop
  - non-object payload -> `ack` and drop
  - post-parse service failure -> `nak`
  - runtime JetStream request -> response over a real NATS container
  - runtime `context.updated` publication over a real NATS container

Remaining Phase 3 work:

- planning compatibility consumers
- orchestration compatibility consumers
- wiring `context.events.updated` into the compatibility write-path and orchestration triggers that emit it in the frozen baseline

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

Stay in Phase 3 and finish the remaining external NATS consumers.

The next implementation cut should produce:

1. planning compatibility consumers
2. orchestration compatibility consumers
3. parity tests for their publication or `ack` or `nak` behavior without expanding the adapter into a god file
