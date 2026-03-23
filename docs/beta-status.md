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
| `UpdateContext` | **Experimental** | Accepts commands but does not persist changes to the graph projection. Returns a synthetic version and content hash. No revision conflict detection. No idempotency enforcement. Warnings in the response document what is missing. |

## ContextAdminService (gRPC)

| RPC | Status | Notes |
|-----|--------|-------|
| `GetProjectionStatus` | Production-ready | |
| `ReplayProjection` | Production-ready | |
| `GetBundleSnapshot` | Production-ready | |
| `GetGraphRelationships` | Production-ready | |
| `GetRehydrationDiagnostics` | Production-ready | |

## HTTP Admin

**Status: Placeholder.** The `rehydration-transport-http-admin` crate is not functional.
The gRPC `ContextAdminService` is the real admin surface.

## Known Limitations

- Token counting uses whitespace splitting, not model-aware tokenization.
- `UpdateContext` is aspirational; real persistence, revision control, and
  idempotency are planned for a future phase.
- `RehydrateSession` does not implement timeline or summary filtering.
- Scope validation has no authorization backend; `GetContext` omits it entirely
  and `ValidateScope` operates as a standalone set-comparison utility.
