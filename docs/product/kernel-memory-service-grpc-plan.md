# KernelMemoryService gRPC API-First Plan

Status: current implementation slice for `feat/kernel-memory-service-grpc`.

This document defines the clean API-first slice before the MCP migration. The
goal is to promote Kernel Memory Protocol (KMP) into domain, application, and
typed gRPC code first. MCP is deliberately delayed until the kernel memory API
is coherent and tested on its own.

The important boundary is the public memory contract. Existing query and
command services may be used behind that boundary while this slice is delivered,
but their generic `GetContext`, `GetContextPath`, `GetNodeDetail`, and
`UpdateContext` shapes must not leak into the public KMP API.

## Ground Truth Read

The current repository state is:

- `ContextQueryService` and `ContextCommandService` are the stable v1beta1
  node-centric gRPC services.
- `ContextQueryService` exposes `GetContext`, `GetContextPath`,
  `GetNodeDetail`, `RehydrateSession`, and `ValidateScope`.
- `ContextCommandService` exposes `UpdateContext`.
- KMP JSON fixtures already exist under
  `api/examples/kernel/v1beta1/kmp`.
- The previous MCP-oriented spike is not the target architecture for this
  slice.
- The application command path has memory projection support for
  `memory_dimension`, `memory_entry`, `memory_relation`, and
  `memory_evidence` changes.
- `GraphBatch` is a separate experimental ingestion boundary for
  model-generated graph materialization. It must remain separate from KMP
  memory ingest.

## Deferred MCP Binding

This is the legacy behavior to avoid when MCP is resumed. It is not part of the
current gRPC-only cut:

| MCP tool | Current live binding |
|:---------|:---------------------|
| `kernel_ingest` | `ContextCommandService.UpdateContext` |
| `kernel_wake` | `ContextQueryService.GetContext` |
| `kernel_ask` | `ContextQueryService.GetContext` |
| `kernel_goto` | `ContextQueryService.GetContext` |
| `kernel_near` | `ContextQueryService.GetContext` |
| `kernel_rewind` | `ContextQueryService.GetContext` |
| `kernel_forward` | `ContextQueryService.GetContext` |
| `kernel_trace` | `ContextQueryService.GetContextPath` |
| `kernel_inspect` | `ContextQueryService.GetNodeDetail` |

The durable surface for this cut is `KernelMemoryService`. MCP must not own
temporal or multidimensional traversal logic.

## Target Service

Add `api/proto/underpass/rehydration/kernel/v1beta1/memory.proto`:

```proto
service KernelMemoryService {
  rpc Ingest(IngestRequest) returns (IngestResponse);
  rpc Wake(WakeRequest) returns (WakeResponse);
  rpc Ask(AskRequest) returns (AskResponse);
  rpc Goto(TemporalMoveRequest) returns (TemporalMoveResponse);
  rpc Near(TemporalNearRequest) returns (TemporalMoveResponse);
  rpc Rewind(TemporalMoveRequest) returns (TemporalMoveResponse);
  rpc Forward(TemporalMoveRequest) returns (TemporalMoveResponse);
  rpc Trace(TraceRequest) returns (TraceResponse);
  rpc Inspect(InspectRequest) returns (InspectResponse);
}
```

This is an additive service in the existing
`underpass.rehydration.kernel.v1beta1` package. It does not rename or replace
the existing v1beta1 services.

## Contract Rules

- KMP messages use memory language: memory, dimension, entry, coordinate,
  relation, evidence, proof, temporal cursor.
- Public KMP requests must not expose `ContextChange`, `payload_json`,
  `root_node_id` command wording, or `UpdateContext` preconditions.
- gRPC should use typed protobuf fields. JSON fixtures remain the MCP/NATS
  binding examples and map deterministically to the proto shape.
- Timestamps in gRPC use `google.protobuf.Timestamp`; JSON fixtures keep
  RFC3339 strings.
- Metadata is typed as string maps only where it is intentionally opaque
  caller metadata.
- Invalid caller input fails with `INVALID_ARGUMENT`.
- Missing kernel state fails with `NOT_FOUND` where the requested anchor or
  reference does not exist.
- Projection or storage conflicts fail with `ABORTED`.
- No silent compatibility fallback: once MCP live mode moves to
  `KernelMemoryService`, it must fail clearly if the service is unavailable.
- `Ask` must not invent a generated answer. Until a real answer engine exists,
  it returns deterministic rendered memory context plus evidence/proof.

## Proto Shape

The proto should mirror the transport-neutral KMP schema, with explicit shared
messages:

| Message | Purpose |
|:--------|:--------|
| `IngestRequest` | `about`, typed `Memory`, typed `MemoryProvenance`, `idempotency_key`, `dry_run`. |
| `Memory` | Repeated `MemoryDimension`, `MemoryEntry`, `MemoryRelation`, `MemoryEvidence`. |
| `MemoryDimension` | `id`, `kind`, `title`, `metadata`. |
| `MemoryEntry` | `id`, `kind`, `text`, repeated `TemporalCoordinate`, `metadata`. |
| `TemporalCoordinate` | `dimension`, `scope_id`, `occurred_at`, `observed_at`, `ingested_at`, `valid_from`, `valid_until`, `sequence`, `rank`, `metadata`. |
| `MemoryRelation` | `from`, `to`, `rel`, `semantic_class`, `why`, `evidence`, `confidence`, `sequence`. |
| `MemoryEvidence` | `id`, `supports`, `text`, `source`, `time`, `metadata`. |
| `MemoryProvenance` | `source_kind`, `source_agent`, `observed_at`, `correlation_id`, `causation_id`. |
| `MemoryBudget` | `tokens`, `detail`, `depth`. |
| `DimensionSelection` | `mode`, `include`, `exclude`. |
| `TemporalCursor` | `ref`, `time`, or `sequence`; exactly one should be accepted. |
| `TemporalWindow` | Entry and time window controls for `Near`. |
| `TemporalInclude` | `evidence`, `relations`, `raw_refs` include flags. |
| `Proof` | `path`, `evidence`, `conflicts`, `missing`, `confidence`. |
| `TraceRequest` | `from`, `to`, `goal`, include flags. |
| `InspectRequest` | `ref`, include flags for links, details, and raw state. |

Response families:

| Response | Required shape |
|:---------|:---------------|
| `IngestResponse` | Summary, accepted counts, memory id, read-after-write readiness, warnings. |
| `WakeResponse` | Summary, wake payload, proof, warnings. |
| `AskResponse` | Summary, optional answer, evidence reasons, proof, warnings. |
| `TemporalMoveResponse` | Summary, resolved cursor, coverage, temporal entries, proof, warnings. |
| `TraceResponse` | Summary, relationship trace, warnings. |
| `InspectResponse` | Summary, inspected object, incoming/outgoing links, evidence, warnings. |

Critical enums to define in `memory.proto`:

- `MemoryConfidence`: `HIGH`, `MEDIUM`, `LOW`, `UNKNOWN`.
- `MemorySemanticClass`: structural, causal, motivational, procedural,
  evidential, constraint.
- `DimensionSelectionMode`: all, only, except.
- `TemporalDirection`: goto, near, rewind, forward.
- `MemoryDetailLevel`: compact, balanced, full.
- `AnswerPolicy`: evidence-or-unknown, show-conflicts, best-effort.

## Hexagonal Architecture

KMP behavior should follow the existing hexagonal boundaries:

Domain owns memory traversal concepts and rules:

```text
crates/rehydration-domain/src/value_objects/
  dimension_selection.rs
  temporal_coordinate.rs
  temporal_cursor.rs

crates/rehydration-domain/src/model/temporal_memory/
  mod.rs
  axis_key.rs
  extract.rs
  position.rs
  select.rs
```

Application owns orchestration over existing ports:

```text
crates/rehydration-application/src/memory/
  mod.rs
  types.rs
  ingest.rs
  service.rs
```

Responsibilities:

- validate typed KMP memory requests;
- translate typed KMP ingest into the current internal command path;
- orchestrate wake, ask, trace, inspect, and temporal reads over existing
  application query/command ports;
- call the domain temporal traversal service after loading a bundle;
- keep transport, MCP, and persistence details outside domain code.

Transport code should stay thin:

```text
crates/rehydration-transport-grpc/src/transport/memory_grpc_service_v1beta1.rs
crates/rehydration-transport-grpc/src/transport/proto_mapping_v1beta1/memory_mapping.rs
```

Responsibilities:

- map proto requests to application memory commands;
- map application memory results back to proto responses;
- map errors to tonic statuses using the same rules as existing transport.

## Ingest Path

`KernelMemoryService.Ingest` is KMP memory ingest. It is not GraphBatch ingest.

The first implementation can reuse the existing event-store and memory
projection behavior by translating typed KMP input into an internal
`UpdateContextCommand` with `memory_*` changes. That translation must be a
single isolated adapter inside the application layer.

Required validations before the internal command is created:

- `about` is non-empty and valid as a kernel case id.
- `idempotency_key` is non-empty.
- dimensions are non-empty and dimension ids are unique, unless entries target
  already materialized memory dimensions in the read model.
- entries are non-empty and entry ids are unique.
- every entry coordinate references a known dimension/scope id.
- temporal coordinate numbers are positive.
- relation endpoints are either submitted in the same request or already
  materialized in the read model; missing endpoints fail instead of relying on
  placeholder projection.
- relations use a known semantic class.
- non-structural relations include enough proof material: `why` or `evidence`,
  plus confidence.
- evidence ids are unique and `supports` references are submitted or existing
  refs when present.

Acceptance semantics:

- `dry_run=true` validates and returns a response without writing.
- accepted live writes preserve current idempotency behavior.
- `read_after_write_ready=true` is only returned when the synchronous memory
  projection mutation path completed successfully.
- storage conflicts are not downgraded to warnings.

## Read And Temporal Path

`Wake`, `Ask`, `Goto`, `Near`, `Rewind`, and `Forward` read through the
application query layer, then assemble KMP results.

Temporal traversal rules:

- temporal positions come from `contains_entry` relationship explanations;
- sorting uses timestamp priority first, then sequence, rank, and ref id;
- dimension selection filters positions before cursor movement;
- `goto` returns entries at or before the cursor;
- `near` returns bounded before/after windows around the cursor;
- `rewind` returns entries strictly before the cursor;
- `forward` returns entries strictly after the cursor;
- malformed cursors fail with `INVALID_ARGUMENT`;
- absent temporal positions are returned as proof `missing`, not fabricated.

`Ask` rules:

- generated answers are not part of this slice;
- `answer` is the deterministic rendered memory context returned by
  `GetContext`;
- `answer_policy=evidence_or_unknown` returns `UNKNOWN` when the selected
  context has no evidence;
- `answer_policy=show_conflicts` keeps deterministic context and exposes
  conflicts when the proof model has them;
- `answer_policy=best_effort` returns deterministic context even without
  evidence;
- the method returns evidence, path, conflicts, missing data, and confidence
  without pretending to have run a generative answer engine.

## Trace And Inspect Path

`Trace` reads a path between two refs through the existing path query behavior
and returns KMP relationship proof.

`Inspect` reads node detail for one ref. It honors `details=false`. It must not
pretend to support link directions or raw expansion that the underlying reader
cannot supply. If a caller explicitly requests incoming links, outgoing links,
or raw expansion before the reader can serve them, the method fails clearly.

## Next Slice: MCP Migration

After `KernelMemoryService` is registered, tested, and smoke-tested as a gRPC
service, live MCP mode should become a client of that service. This is
explicitly outside the current gRPC-only cut.

MCP live mode should keep only:

- JSON-RPC request parsing;
- MCP tool schema handling;
- JSON/proto conversion;
- structuredContent conversion;
- TLS endpoint configuration.

MCP live mode should remove direct client calls to:

- `ContextCommandServiceClient`;
- `ContextQueryServiceClient`.

Fixture mode remains explicit with `REHYDRATION_MCP_BACKEND=fixture`.

## Test Plan

Contract tests:

- descriptor set includes `memory.proto`;
- `KernelMemoryService` method names are locked;
- critical message field names are locked;
- temporal presence fields are locked as `optional` so zero can fail instead
  of being treated as absent;
- KMP fixtures identify `memory.proto` as the typed gRPC binding for this cut.

Application tests:

- typed ingest validates dimensions, entries, relations, evidence, and
  idempotency;
- typed ingest produces memory projection mutations through the existing
  command application port;
- temporal traversal covers all four directions, ref cursor, time cursor,
  sequence cursor, dimension `all`, `only`, and `except`;
- ask never returns an invented generated answer;
- inspect fails fast for unsupported explicit include flags.

Transport tests:

- every `KernelMemoryService` method has a direct gRPC service test;
- server descriptors/accessors expose the new service;
- error mapping matches existing tonic status conventions;
- TLS/mTLS server tests continue to pass with the additional service.

Next-slice MCP tests:

- live MCP test server implements `KernelMemoryService` only;
- live MCP tests fail if the adapter still calls `ContextQueryService` or
  `ContextCommandService`;
- dry-run ingest remains local and does not call gRPC.

Current or follow-up integration tests:

- real kernel container journey ingests KMP memory via the typed gRPC service;
- wake reads the ingested memory back;
- forward or rewind proves temporal traversal from real `contains_entry`
  relations;
- trace and inspect run against seeded kernel data.

## Deployment And Live Smoke

After the gRPC-only code and tests pass:

1. Build and push a branch image.
2. Deploy with Helm using the existing chart and runtime values.
3. Verify the public endpoint, not a port-forwarded endpoint.
4. Run typed gRPC calls through `KernelMemoryService`:
   - `Ingest`;
   - `Wake`;
   - `Forward` or `Rewind`;
   - `Trace`;
   - `Inspect`.

MCP rebuild/reinstall belongs to the next slice.

The public endpoint smoke target remains:

```text
https://rehydration-kernel.underpassai.com
```

## Documentation Updates

After implementation:

- `docs/beta-status.md` records `KernelMemoryService` maturity separately.
- `docs/migration/kernel-node-centric-integration-contract.md` lists the
  additive service and states that lower-level query/command services remain.
- `docs/operations/mcp-stdio.md` describes live MCP as
  `KernelMemoryService`-backed.
- `crates/rehydration-mcp/README.md` removes the old live binding table.
- KMP fixtures reference `memory.proto` as the typed gRPC binding.

## Non-Goals

- Do not implement NATS `kernel.memory.ingest` in this slice.
- Do not implement GraphBatch transport in this slice.
- Do not publish crates.io packages in this slice.
- Do not remove `ContextQueryService` or `ContextCommandService`.
- Do not introduce compatibility behavior that silently calls the old services
  after MCP live mode migrates.
- Do not claim generated `Ask` answers until a real answer implementation is
  added and tested.

## Definition Of Done

Current gRPC-only cut:

- `KernelMemoryService` exists in proto and descriptor tests.
- The gRPC server registers the service.
- All nine methods compile, are callable, and have focused tests.
- Temporal and multidimensional traversal are domain-owned and covered by
  domain tests.
- KMP behavior is application-owned, not MCP-owned.
- CI passes.
- A real deployed public endpoint smoke proves ingest, read, temporal
  traversal, trace, and inspect through the typed service.

Next MCP cut:

- MCP live mode calls `KernelMemoryService`.
- MCP no longer calls `ContextQueryService` or `ContextCommandService`
  directly for KMP moves.
- Fixture mode remains explicit.
