# Plan: Graph Explorer

**Status:** proposed
**Priority:** P0
**Date:** 2026-03-18
**Related:**
- [`docs/REQUIREMENTS_GRAPH_EXPLORER.md`](./REQUIREMENTS_GRAPH_EXPLORER.md)
- [`docs/BUG_DEPTH_TRAVERSAL.md`](./BUG_DEPTH_TRAVERSAL.md)

## Goal

Deliver a kernel-owned read surface that supports:

- graph traversal beyond 1 hop
- rehydration from any node in the graph
- node detail lookup for an interactive explorer
- a demoable end-to-end explorer journey against a real seeded graph

## Decisions

### 1. Native kernel API can evolve; compatibility API stays frozen

The explorer should consume the native kernel API, not the frozen
`fleet.context.v1` compatibility shell.

Implication:

- `underpass.rehydration.kernel.v1alpha1` may add explorer-specific fields and
  RPCs
- `fleet.context.v1.GetGraphRelationships.depth` stays clamped to `1..3` until
  there is an explicit compatibility decision to change that contract

### 2. Replace the hardcoded `1..3` kernel limit with a server guardrail

The requirement says "no artificial caps", but a literal unbounded traversal is
not a safe server contract for cyclic or unexpectedly large graphs.

Plan:

- remove the hardcoded `1..3` limit from the kernel-owned path
- introduce a kernel-native default depth, recommended `10`
- introduce a kernel-native maximum traversal depth, recommended `25`
- clamp only at the kernel-native guardrail, not at the old compatibility bound

### 3. `GetNodeDetail` must compose stores

The current Valkey detail projection only stores:

- `node_id`
- `detail`
- `content_hash`
- `revision`

Title, kind, summary, status, labels, and properties live in Neo4j. If the
explorer wants a full node panel, `GetNodeDetail` cannot be a pure Valkey read
with the current model.

Plan:

- `GetNodeDetail` will compose Neo4j node projection + Valkey node detail
- Valkey remains the source for expanded detail text and detail revision
- Neo4j remains the source for title and properties unless the projection model
  is expanded later

## Phase 1: Traversal Foundation

### Outcome

The kernel can load multi-hop neighborhoods and all bundle-producing read paths
stop being limited to 1 hop.

### Scope

- add `depth` to `GraphNeighborhoodReader`
- propagate depth through:
  - `GetGraphRelationships`
  - `GetContext`
  - `RehydrateSession`
  - bundle snapshot reads
  - diagnostics reads
- update Neo4j traversal query to variable-depth paths
- deduplicate relationships across multi-hop expansion

### Files

- `crates/rehydration-domain/src/repositories/graph_neighborhood_reader.rs`
- `crates/rehydration-adapter-neo4j/src/adapter/load_neighborhood.rs`
- `crates/rehydration-adapter-neo4j/src/adapter/queries/load_neighborhood_query.rs`
- `crates/rehydration-application/src/queries/graph_relationships.rs`
- `crates/rehydration-application/src/queries/node_centric_projection_reader.rs`
- `crates/rehydration-application/src/queries/get_context.rs`
- `crates/rehydration-application/src/queries/rehydrate_session.rs`
- `crates/rehydration-application/src/queries/bundle_snapshot.rs`
- `crates/rehydration-application/src/queries/rehydration_diagnostics.rs`

### Tests

- unit test: `GetGraphRelationships` forwards requested depth
- unit test: bundle reads use the kernel default depth
- Neo4j integration test: depth `1` vs `3` returns different neighborhood sizes
- integration test: a 3-level seeded graph is fully returned at depth `3`

## Phase 2: Native Explorer Read Contract

### Outcome

The native kernel query surface exposes depth explicitly for explorer-style
reads, without changing the frozen compatibility shell.

### Scope

- add `depth` to native `GetContextRequest`
- treat `depth=0` as "use kernel default depth"
- keep compatibility `GetContext` unchanged
- keep compatibility `GetGraphRelationships.depth` clamped to `1..3`
- add shared transport helpers for native depth default and max guardrail

### Files

- `api/proto/underpass/rehydration/kernel/v1alpha1/query.proto`
- `crates/rehydration-transport-grpc/src/transport/query_grpc_service.rs`
- `crates/rehydration-transport-grpc/src/transport/tests.rs`
- native request/response fixtures under `api/examples` if needed

### Tests

- gRPC transport test: `GetContext(depth=0)` uses kernel default depth
- gRPC transport test: `GetContext(depth=N)` forwards the requested depth
- regression test: compatibility `GetGraphRelationships` still clamps to `1..3`

## Phase 3: `GetNodeDetail`

### Outcome

The explorer can open any node and retrieve a full node panel in one RPC.

### Scope

- add `GetNodeDetail` to native `ContextQueryService`
- define a response shape that reflects current data ownership
- read node projection metadata from Neo4j
- read expanded detail text from Valkey
- return `NOT_FOUND` when the node does not exist in either store

### Proposed response

```protobuf
rpc GetNodeDetail(GetNodeDetailRequest) returns (GetNodeDetailResponse);

message GetNodeDetailRequest {
  string node_id = 1;
}

message GetNodeDetailResponse {
  string node_id = 1;
  string node_kind = 2;
  string title = 3;
  string summary = 4;
  string status = 5;
  repeated string labels = 6;
  map<string, string> properties = 7;
  string detail = 8;
  string content_hash = 9;
  uint64 revision = 10;
}
```

### Files

- `api/proto/underpass/rehydration/kernel/v1alpha1/query.proto`
- `crates/rehydration-application/src/queries/*`
- `crates/rehydration-transport-grpc/src/transport/query_grpc_service.rs`
- new query adapter or application composition around Neo4j + Valkey readers

### Tests

- unit test: node exists in both stores
- unit test: node exists in Neo4j but has no Valkey detail yet
- unit test: missing node returns not found
- transport test: `GetNodeDetail` maps success and not-found correctly

## Phase 4: Explorer E2E and Demo

### Outcome

The repo proves the explorer journey against a real, multi-level graph.

### Scope

- add a deeper seed than the current starship path
- prove:
  - full graph load from root
  - zoom into mid-level node
  - leaf rehydration
  - node detail lookup
  - rendered context changes when root changes

### Suggested scenario

Use a graph with:

- at least 4 levels
- sibling branches
- cross-branch dependency edges
- detail content for root, mid-level, and leaf nodes

The current starship incident graph is a good seed base, but it should be
extended to include a deeper subtree specifically for explorer navigation.

### Tests

- container-backed integration test for multi-hop `GetContext`
- container-backed integration test for `GetNodeDetail`
- cluster demo script for:
  - root load
  - zoom to mid-level node
  - open detail panel
  - zoom to leaf node

## Out Of Scope

- changing the external `fleet.context.v1` contract in this slice
- adding write-side explorer mutations
- moving all node metadata into Valkey
- UI implementation in this repo

## Risks

- variable-depth traversal can blow up quickly on dense graphs if the server
  guardrail is too high
- cycles and cross-links can multiply relationship rows if the query is not
  deduplicated carefully
- changing compatibility depth behavior in the same slice would create an
  avoidable migration blast radius
- a pure-Valkey `GetNodeDetail` is not realistic with the current projection
  model

## Exit Criteria

- native `GetContext` supports explicit depth
- bundle-producing reads no longer stop at 1 hop
- native `GetNodeDetail` exists and is transport-tested
- explorer scenario passes on a deep seeded graph
- compatibility depth behavior remains unchanged unless separately approved
