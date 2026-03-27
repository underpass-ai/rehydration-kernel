# v1beta1 RPC Maturity Matrix

Status of each RPC in the `v1beta1` contract surface.

## Path to v1

The kernel targets a stable `v1` contract once these conditions are met:

| Condition | Status |
|:----------|:-------|
| All deprecated proto fields removed or implemented | Pending — 8 fields deprecated |
| Authorization backend for scope validation | Not started |
| Timeline and summary filtering in RehydrateSession | Not started |
| Quality metrics in all render paths (including RehydrateSession) | Partial — GetContext and GetContextPath only |
| Full benchmark matrix validated (3 agents x 3 judges x 4 noise) | Planned — see [ROADMAP_MASTER.md](research/ROADMAP_MASTER.md) |
| Async contract: `context.bundle.generated` actually emitted by runtime | Not started — contract-only today |
| OTLP mTLS to OTel Collector | In progress — plaintext today |
| Neo4j client mTLS (2FA since Neo4j 5.19+) | Not started — adapter only passes CA today |
| Grafana: disable anonymous admin access by default | Not started — development default is anonymous=Admin |
| Event store atomic CAS for optimistic concurrency | Not started — check-then-act today |
| Breaking change window communicated to consumers | Not started |

Until then, the `v1beta1` contract is **stable for current fields** — no breaking
changes to fields that are implemented. Deprecated fields may be removed in `v1`.

## ContextQueryService

| RPC | Status | Notes |
|-----|--------|-------|
| `GetContext` | Production-ready | Proto fields `phase`, `work_item_id`, `render_format`, and `include_debug_sections` are deprecated and do not alter query behavior in v1beta1. `scope_validation` is `None` (no authorization backend). |
| `GetContextPath` | Production-ready | Returns `NOT_FOUND` when neither the path nor the target node exist. |
| `GetNodeDetail` | Production-ready | |
| `RehydrateSession` | Production-ready | `include_timeline` and `include_summaries` are deprecated (reserved for future use, currently ignored). `timeline_window` is echoed in the response but does not filter events. `snapshot_ttl` is required when `persist_snapshot` is true. |
| `ValidateScope` | Production-ready | Standalone set comparison. Proto fields `role` and `phase` are deprecated and do not affect the comparison — only `required_scopes` and `provided_scopes` are evaluated. |

## ContextCommandService

| RPC | Status | Notes |
|-----|--------|-------|
| `UpdateContext` | Production-ready | Persists full domain events (changes, requested_by, occurred_at) to the context event store as JSON. Optimistic concurrency via `expected_revision`. Content hash validated via `expected_content_hash`. Idempotency via `idempotency_key`. Returns `ABORTED` on revision or hash conflict. Proto field `persist_snapshot` is accepted but not acted upon — snapshot persistence is the responsibility of the query path (`RehydrateSession`). |

## Async Contract (NATS JetStream)

Event channels defined in [`context-projection.v1beta1.yaml`](../api/asyncapi/context-projection.v1beta1.yaml).

| Channel | Direction | Status | Notes |
|:--------|:----------|:-------|:------|
| `graph.node.materialized` | Subscribe (kernel consumes) | Production-ready | Materializes nodes + relationships into Neo4j. Full `EventEnvelope` with 7 required fields + `data` payload. `related_nodes` carries explanatory metadata (semantic_class, rationale, method, decision_id, caused_by_node_id, evidence, confidence, sequence) |
| `node.detail.materialized` | Subscribe (kernel consumes) | Production-ready | Materializes extended detail into Valkey. Requires `node_id`, `detail`, `content_hash`, `revision` |
| `context.bundle.generated` | Publish (kernel emits) | **Contract only** | Defined in AsyncAPI but **not emitted by the kernel runtime**. Test fixtures simulate it. Implementation pending |

All channels use the shared `EventEnvelope` schema: `event_id`, `correlation_id`,
`causation_id`, `occurred_at`, `aggregate_id`, `aggregate_type`, `schema_version`.

Subject prefix is configurable via `REHYDRATION_EVENTS_PREFIX` (default: `rehydration`).

## Removed

- **ContextAdminService** — removed entirely. The admin RPCs (`GetProjectionStatus`, `ReplayProjection`, `GetBundleSnapshot`, `GetGraphRelationships`, `GetRehydrationDiagnostics`) were placeholder-backed and have been deleted.
- **HTTP admin** — never implemented; crate deleted.

## Quality Metrics (v1beta1)

`BundleQualityMetrics` is a domain value object computed on every `GetContext` and
`GetContextPath` render. Five metrics with invariant validation:

| Metric | Description |
|--------|-------------|
| `raw_equivalent_tokens` | Flat text baseline token count |
| `compression_ratio` | raw / rendered ratio (>1.0 = compression) |
| `causal_density` | Fraction of explanatory relationships |
| `noise_ratio` | Fraction of noise/distractor nodes |
| `detail_coverage` | Fraction of nodes with detail |

Delivered through the `QualityMetricsObserver` hexagonal port with two active adapters:
- **OTel**: 5 histograms via OTLP (see [observability.md](observability.md))
- **Tracing**: structured JSON logs for Loki/Grafana

**In progress**: `RehydrateSession` does not yet emit quality metrics — per-role
rendering is planned (see [ROADMAP_MASTER.md](research/ROADMAP_MASTER.md)).

## Proto Fields Accepted but Ignored

These fields exist in the wire format for forward compatibility but have **no effect** in v1beta1:

| RPC | Field | Status |
|:----|:------|:-------|
| `GetContext` | `phase`, `work_item_id`, `render_format`, `include_debug_sections` | Deprecated. Do not alter query behavior |
| `ValidateScope` | `role`, `phase` | Deprecated. Only `required_scopes` / `provided_scopes` are evaluated |
| `RehydrateSession` | `include_timeline`, `include_summaries` | Reserved for future use. `timeline_window` is echoed but does not filter |
| `UpdateContext` | `persist_snapshot` | Accepted but not acted upon — snapshot persistence is owned by `RehydrateSession` |

## Known Limitations

**Not implemented in v1beta1:**

- **No authorization backend** — `ValidateScope` is a pure set-comparison utility, not an access control gate. `GetContext` does not invoke scope validation at all
- **No timeline filtering** — `RehydrateSession` echoes `timeline_window` but does not filter events by time range
- **No summary filtering** — `include_summaries` is a no-op
- **`RehydrateSession` quality metrics** — not yet emitted; per-role rendering planned
- **`context.bundle.generated` not emitted** — defined in AsyncAPI contract but the kernel runtime does not publish this event. Test fixtures simulate it for downstream integration tests
- **Single token estimator** — `cl100k_base` BPE via `tiktoken-rs` for all counting. No model-specific estimator selection
- **Event store concurrency** — optimistic concurrency uses check-then-act (not atomic CAS). Under high concurrent writes to the same `(root_node_id, role)`, the second writer can silently overwrite the first. CAS via JetStream `expected_last_subject_sequence` is planned
- **Idempotency outcome** — outcome publish is fire-and-forget. If it fails, retries are treated as new requests
