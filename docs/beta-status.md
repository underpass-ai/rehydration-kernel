# v1beta1 RPC Maturity Matrix

Status of each RPC in the `v1beta1` contract surface.

## ContextQueryService

| RPC | Status | Notes |
|-----|--------|-------|
| `GetContext` | Production-ready | `scope_validation` is intentionally `None`; no authorization backend exists. Use `ValidateScope` separately. |
| `GetContextPath` | Production-ready | Returns `NOT_FOUND` when neither the path nor the target node exist. |
| `GetNodeDetail` | Production-ready | |
| `RehydrateSession` | Production-ready | `include_timeline` and `include_summaries` are reserved for future use and currently ignored. `snapshot_ttl` is required when `persist_snapshot` is true. |
| `ValidateScope` | Production-ready | Standalone scope comparison. No authorization backend integration. |

## ContextCommandService

| RPC | Status | Notes |
|-----|--------|-------|
| `UpdateContext` | Production-ready | Persists events to the context event store with optimistic concurrency (revision checking) and idempotency key deduplication. Content hash is computed from actual change payloads. Returns `ABORTED` on revision conflict. |

## ContextAdminService (gRPC)

| RPC | Status | Notes |
|-----|--------|-------|
| `GetProjectionStatus` | Production-ready | |
| `ReplayProjection` | Production-ready | |
| `GetBundleSnapshot` | Production-ready | |
| `GetGraphRelationships` | Production-ready | |
| `GetRehydrationDiagnostics` | Production-ready | |

## Known Limitations

- Token counting uses `cl100k_base` BPE tokenizer (via `tiktoken-rs`).
- `RehydrateSession` does not implement timeline or summary filtering.
- Scope validation has no authorization backend; `GetContext` omits it entirely
  and `ValidateScope` operates as a standalone set-comparison utility.
- Context event store has adapters for Valkey and NATS JetStream.
- HTTP admin surface has been removed. The gRPC `ContextAdminService` is the
  only admin interface.
- OpenTelemetry traces exported via OTLP when `OTEL_EXPORTER_OTLP_ENDPOINT` is set.
