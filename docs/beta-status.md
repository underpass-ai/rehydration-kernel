# v1beta1 RPC Maturity Matrix

Status of each RPC in the `v1beta1` contract surface.

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

## Removed

- **ContextAdminService** — removed entirely. The admin RPCs (`GetProjectionStatus`, `ReplayProjection`, `GetBundleSnapshot`, `GetGraphRelationships`, `GetRehydrationDiagnostics`) were placeholder-backed and have been deleted.
- **HTTP admin** — never implemented; crate deleted.

## Known Limitations

- Token counting uses `cl100k_base` BPE tokenizer (via `tiktoken-rs`). Both global and per-section counts use the same estimator.
- `RehydrateSession` does not implement timeline or summary filtering.
- Scope validation has no authorization backend; `GetContext` omits it entirely and `ValidateScope` is a pure set-comparison utility.
- Context event store has adapters for Valkey and NATS JetStream. Backend selected via `REHYDRATION_EVENT_STORE_BACKEND`.
- Relationship rendering is ordered by semantic salience: causal > motivational > evidential > constraint > procedural > structural.
- OpenTelemetry traces and metrics exported via OTLP when `OTEL_EXPORTER_OTLP_ENDPOINT` is set.
