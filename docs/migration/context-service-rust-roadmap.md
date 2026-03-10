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
The next milestone is freezing and documenting how external products should
adapt to the node-centric boundary.

## Direction Update

From this point forward, `swe-ai-fleet`-specific compatibility should move out
of this repo.

This repo should not keep expanding with:

- `planning.*`
- `orchestration.*`
- `story`
- `task`
- `project`
- other fleet-specific nouns

Those concerns belong in an anti-corruption layer inside `swe-ai-fleet`.

See:

- [`swe-ai-fleet-node-centric-integration-strategy.md`](./swe-ai-fleet-node-centric-integration-strategy.md)
- [`swe-ai-fleet-integration-checklist.md`](./swe-ai-fleet-integration-checklist.md)

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

Status: `phase 0 complete, phase 1 complete, phase 2 complete, phase 3 complete for kernel-owned boundary`

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

Status: `complete for kernel-owned boundary`

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

Remaining work previously associated with fleet-specific subjects is now
externalized to `swe-ai-fleet`.

Exit gate:

- async golden tests pass
- envelope behavior matches the baseline
- public subjects are implemented without turning the NATS adapter into a god file

Status note: complete for this repo

Delivered:

- request or reply parity for kernel-owned async subjects
- real JetStream runtime wiring
- container-backed runtime integration coverage
- `context.events.updated` publication support at the kernel edge

### Phase 4: swe-ai-fleet Integration Adapter

Status: `pending and external to this repo`

Goal:

- adapt `swe-ai-fleet` legacy concepts to the node-centric kernel contract

Scope:

- fleet-side request mapping
- fleet-side response mapping
- fleet-side async bridge
- fleet-side subject translation
- fleet-side shadow comparison

Deliverables:

- anti-corruption layer in `swe-ai-fleet`
- mapping matrix from legacy nouns to node-centric structures
- shadow comparison harness in `swe-ai-fleet`
- fleet-owned rollout switches

Exit gate:

- no fleet-specific nouns are added to kernel domain or transport
- `swe-ai-fleet` can call the kernel through a narrow adapter
- legacy-to-node-centric drift is measurable outside this repo

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

Do not add `planning.*` or `orchestration.*` consumers here.

The next implementation cut should produce:

1. a frozen node-centric integration contract for external consumers
2. the strategic integration document for `swe-ai-fleet`
3. a migration checklist for moving legacy compatibility out of this repo

Artifacts now available:

- [`swe-ai-fleet-node-centric-integration-strategy.md`](./swe-ai-fleet-node-centric-integration-strategy.md)
- [`swe-ai-fleet-integration-checklist.md`](./swe-ai-fleet-integration-checklist.md)
