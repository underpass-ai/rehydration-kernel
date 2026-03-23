# Bug: Graph Traversal Limited to Depth 1

**Severity:** HIGH — blocks interactive graph exploration and demo rehydration phase
**Reported:** 2026-03-18
**Affects:** GetContext, GetGraphRelationships, RehydrateSession

## Summary

The kernel returns only 1 level of neighbors for any graph query, regardless of graph depth or client request. The `depth` parameter in `GetGraphRelationships` is accepted but silently ignored. This makes it impossible to:

1. Render a full task graph (e.g., root → subtasks → sub-subtasks)
2. Build an interactive graph explorer that drills into nodes
3. Show node details from Valkey when visiting a specific node

## Reproduction

```bash
# Neo4j has a 3-level graph:
#   root → task-A → subtask-1
#                  → subtask-2
#        → task-B → subtask-3

grpcurl -plaintext -d '{
  "root_node_id": "node:mission:engine-core-failure",
  "role": "implementer",
  "token_budget": 4000
}' localhost:50054 \
  underpass.rehydration.kernel.v1beta1.ContextQueryService/GetContext

# Expected: 8 nodes (root + 7 descendants)
# Actual:   3 nodes (root + 2 direct children only)
```

## Root Cause

Three layers enforce the 1-level limit:

### 1. Domain Port — no depth parameter

**File:** `crates/rehydration-domain/src/repositories/graph_neighborhood_reader.rs`

```rust
pub trait GraphNeighborhoodReader {
    fn load_neighborhood(
        &self,
        root_node_id: &str,
        // ← missing: depth: u32
    ) -> impl Future<Output = Result<Option<NodeNeighborhood>, PortError>> + Send;
}
```

The port contract has no way to request deeper traversal.

### 2. Neo4j Cypher — hardcoded 1-hop

**File:** `crates/rehydration-adapter-neo4j/src/adapter/queries/load_neighborhood_query.rs`

```cypher
MATCH (root:ProjectionNode {node_id: $root_node_id})
OPTIONAL MATCH (root)-[:RELATED_TO]-(seed_neighbor:ProjectionNode)
-- ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ fixed 1-hop pattern
```

The Cypher uses a single relationship match pattern. No variable-length path.

### 3. Application — depth parameter is dead code

**File:** `crates/rehydration-application/src/queries/graph_relationships.rs`

```rust
pub struct GetGraphRelationshipsQuery {
    pub node_id: String,
    pub node_kind: Option<String>,
    pub depth: u32,              // ← ACCEPTED
    pub include_reverse_edges: bool,
}

pub async fn execute(&self, query: GetGraphRelationshipsQuery) -> Result<...> {
    let neighborhood = load_existing_neighborhood(
        &self.graph_reader,
        &query.node_id,            // ← depth NOT passed
    ).await?;
}
```

The gRPC transport clamps `depth` to `[1, 3]` but the value never reaches the Cypher query.

## Call Chain

```
gRPC GetContext / GetGraphRelationships
  ↓
Transport: clamp_depth(request.depth) → depth=3
  ↓
Application: GetGraphRelationshipsQuery { depth: 3, ... }
  ↓
Application::execute() → load_existing_neighborhood(graph_reader, node_id)
  ↓                          ↑ depth NOT forwarded
GraphNeighborhoodReader::load_neighborhood(root_node_id)   ← no depth param
  ↓
Neo4j Cypher: (root)-[:RELATED_TO]-(neighbor)              ← hardcoded 1-hop
  ↓
NodeNeighborhood { root, neighbors: [direct only], relations: [1-level only] }
```

## Required Fix

### Option A: Variable-depth Cypher (recommended)

Changes in 3 files:

**1. Domain port** — add `depth` parameter:

```rust
fn load_neighborhood(
    &self,
    root_node_id: &str,
    depth: u32,
) -> impl Future<Output = Result<Option<NodeNeighborhood>, PortError>> + Send;
```

**2. Neo4j Cypher** — use variable-length path:

```cypher
MATCH (root:ProjectionNode {node_id: $root_node_id})
OPTIONAL MATCH path = (root)-[:RELATED_TO*1..3]-(neighbor:ProjectionNode)
WITH root, collect(DISTINCT neighbor) AS neighbors, collect(relationships(path)) AS all_rels
-- ... flatten and dedup relationships
```

The `*1..3` pattern traverses 1 to `depth` hops. The max should come from the clamped depth parameter.

**3. Application** — forward depth:

```rust
let neighborhood = self.graph_reader
    .load_neighborhood(&query.node_id, query.depth)
    .await?;
```

### Option B: Iterative client-side traversal (workaround, not recommended)

The client calls `load_neighborhood` for each node and reconstructs the tree. Works but:
- N+1 query problem (1 gRPC call per node)
- Latency grows linearly with graph size
- Client takes on graph assembly responsibility that belongs to the kernel

## Additional Requirement: Node Detail Lookup

For an interactive graph explorer, the client needs to visit a node and see its full detail stored in Valkey. Today there is no standalone RPC for this.

**Required:** A `GetNodeDetail(node_id)` RPC that reads from the Valkey detail store and returns the node's full content (description, properties, history).

This could be a new RPC on `ContextQueryService`:

```protobuf
rpc GetNodeDetail(GetNodeDetailRequest) returns (GetNodeDetailResponse);

message GetNodeDetailRequest {
  string node_id = 1;
}

message GetNodeDetailResponse {
  string node_id = 1;
  string title = 2;
  string detail = 3;
  string content_hash = 4;
  uint64 revision = 5;
  map<string, string> properties = 6;
}
```

## Files to Modify

| File | Change |
|------|--------|
| `crates/rehydration-domain/src/repositories/graph_neighborhood_reader.rs` | Add `depth: u32` to trait |
| `crates/rehydration-adapter-neo4j/src/adapter/queries/load_neighborhood_query.rs` | Variable-length Cypher |
| `crates/rehydration-adapter-neo4j/src/adapter/load_neighborhood.rs` | Pass depth to query |
| `crates/rehydration-application/src/queries/graph_relationships.rs` | Forward `depth` to port |
| `crates/rehydration-application/src/queries/get_context.rs` | Forward depth (default 3) |
| `crates/rehydration-transport-grpc/src/transport/*/get_context.rs` | Map depth from request |
| All callers of `load_neighborhood` | Add depth argument |
| Proto `query.proto` | Add `GetNodeDetail` RPC |

## Test Data in Neo4j (already seeded)

The `node:mission:engine-core-failure` graph has 3 levels:

```
node:mission:engine-core-failure          (root, mission, AT_RISK)
  ├── node:task:diagnose-anomaly          (task, done)
  ├── node:task:assess-cascade            (task, done)
  │     ├── node:task:direct-engine-repair    (task, abandoned)
  │     └── node:task:hull-first-protocol     (task, active)
  │           ├── node:task:seal-hull             (task, active)
  │           ├── node:task:stabilize-power       (task, pending)
  │           └── node:task:repair-engine-safe    (task, pending)
```

Querying with depth=1 returns 3 nodes. Depth=3 should return all 8.
