# Context Service Rust Roadmap

Status: Active  
Source of truth: [`CONTEXT_SERVICE_RUST_MIGRATION_PLAN.md`](../../CONTEXT_SERVICE_RUST_MIGRATION_PLAN.md)

## Purpose

Turn the RFC into an execution roadmap for this repo.

This roadmap keeps two rules explicit:

1. The internal core of this repo stays node-centric.
2. Compatibility with the existing Context Service is handled at the boundary,
   not by polluting the core with external domain nouns.

## Non-negotiable constraints

- no god objects
- no god files
- DDD first
- hexagonal boundaries
- one main concept per file
- legacy compatibility only at adapters and transport edges
- the core language remains:
  - root node
  - neighbor nodes
  - relationships
  - extended node detail in Valkey

## Current baseline

The repo already has a usable node-centric substrate in `main`:

- graph-native bundle domain
- split application services and use cases
- Neo4j adapter split into focused modules
- Valkey adapter split into focused modules
- NATS adapter no longer implemented as a god file
- gRPC transport split into focused modules
- workspace tests green
- GitHub Actions quality gate green
- SonarCloud quality gate green

That means the engineering substrate is ahead of the migration contract work.

The missing work is not “more refactor”. The missing work is compatibility and
rollout against the existing Context Service contract described in:

- `specs/fleet/context/v1/context.proto`
- `services/context/README.md`

## Status by stream

### Stream A: Internal node-centric core

Status: `done for current foundation`

Delivered:

- graph-native bundle model
- split adapters
- split transport
- test and quality gates in place

This stream continues only when a compatibility phase reveals a real gap.

### Stream B: External contract compatibility

Status: `not started to completion`

Missing:

- exact parity inventory against the existing service contract
- compatibility mapping for every RPC and NATS subject
- explicit edge adapters for legacy request and response shapes
- golden tests against the baseline implementation

### Stream C: Migration rollout

Status: `not started`

Missing:

- dual-run plan
- canary plan
- rollback plan
- shadow comparison harness

## Phase roadmap

### Phase 0: Contract freeze

Status: `in progress`

Goal:

- freeze the observable contract of the existing Context Service before further
  implementation work

Outputs:

- RPC inventory
- NATS subject inventory
- compatibility matrix
- golden test catalog
- explicit reuse boundary for this repo

Inputs:

- [`CONTEXT_SERVICE_RUST_MIGRATION_PLAN.md`](../../CONTEXT_SERVICE_RUST_MIGRATION_PLAN.md)
- `specs/fleet/context/v1/context.proto`
- `services/context/README.md`
- existing docs under `docs/migration/`

Exit gate:

- every public RPC and every public subject classified as:
  - preserved as-is
  - adapted at the edge
  - deferred with explicit reason

### Phase 1: Compatibility shell

Status: `pending`

Goal:

- expose the external Context Service surface in Rust without leaking external
  domain vocabulary into the internal core

Deliverables:

- boundary transport module for `fleet.context.v1`
- edge DTOs and mappers
- compatibility error mapping
- health/readiness bootstrap

Notes:

- internal use cases continue to work with nodes, relationships, and node
  details
- legacy naming is allowed only in transport and adapter modules

Exit gate:

- the Rust service can boot and answer the external API surface, even if some
  methods still route to controlled placeholders

### Phase 2: Read-path parity

Status: `pending`

Goal:

- serve the external read API from the node-centric core

Scope:

- `GetContext`
- `RehydrateSession`
- `ValidateScope`
- `GetGraphRelationships`

Deliverables:

- transport-to-core mapping from external requests to `root_node_id` workflows
- compatibility rendering from graph-native bundle to external response shape
- deterministic scope validation parity
- snapshot and detail loading parity

Exit gate:

- golden read-path tests pass against the baseline
- shadow comparisons show no unacceptable response drift

### Phase 3: Async NATS parity

Status: `pending`

Goal:

- implement the request/reply and event flows required by the existing service

Scope:

- `context.update.request`
- `context.rehydrate.request`
- `context.update.response`
- `context.rehydrate.response`
- `context.events.updated`

Deliverables:

- subject routers per public flow
- request/reply correlation
- idempotent processing policy
- redelivery tests

Exit gate:

- request/reply parity proven in tests
- deduplication behavior documented and verified

### Phase 4: Write-path parity

Status: `pending`

Goal:

- support external write operations while keeping the core node-centric

Deliverables:

- explicit command-side edge mapping
- idempotent graph writes
- version and event publication parity

Exit gate:

- write contract tests pass
- single-writer rollout strategy is documented

### Phase 5: Dual-run and cutover

Status: `pending`

Goal:

- move production traffic safely from the existing implementation to Rust

Deliverables:

- shadow mode plan
- canary percentages and gates
- rollback mechanism
- observability dashboard checklist

Exit gate:

- staged rollout completed with rollback path verified

## Immediate next slices

These are the next slices to execute in order.

1. Finish Phase 0 as a hard inventory, not as a redesign.
2. Create the compatibility shell for the external gRPC surface.
3. Add golden tests for read-path parity before implementing more external
   behavior.

## Work package rules

Every new work package must satisfy all of these rules:

- one responsibility per file
- one use case per file
- one mapper family per file
- one subject router per file when the logic is non-trivial
- one query builder per query family when the query is not trivial
- no adapter may own transport, mapping, orchestration, and persistence logic in
  the same file

## Definition of done for the roadmap

The migration is done only when all of the following are true:

- the external Context Service surface is compatible
- the internal core remains node-centric
- contract tests are green
- quality gate remains green
- rollout and rollback are both proven
