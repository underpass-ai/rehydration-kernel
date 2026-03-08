# Node-Centric Golden Tests

This catalog defines the minimum golden tests for the node-centric rehydration
model in this repo.

## Principles

1. Compare public behavior, not implementation details.
2. Keep the core language node-centric.
3. Validate graph neighborhood, relationships, and Valkey detail explicitly.
4. Do not introduce fixtures based on external domain nouns.

## Exact-match areas

- `root_node_id`
- root node payload
- neighbor node payloads
- relationship payloads
- node detail payloads
- role-specific filtering results
- rendered section ordering when intentionally defined

## Allowed normalization

- timestamps
- generated-at values
- revision counters when built from controlled test seeds

## Required gRPC fixtures

| Fixture ID | Surface | Scenario | Compare |
| --- | --- | --- | --- |
| `grpc-get-context-root-only` | `GetContext` | root node with no neighbors | root node, empty neighbors, rendered root summary |
| `grpc-get-context-with-neighbors` | `GetContext` | root node plus related nodes and node details | nodes, relationships, details, rendered ordering |
| `grpc-get-context-missing-root` | `GetContext` | root node absent | placeholder or empty bundle behavior |
| `grpc-rehydrate-session-single-role` | `RehydrateSession` | one role over one graph neighborhood | bundle content and metadata |
| `grpc-rehydrate-session-multi-role` | `RehydrateSession` | multiple roles over same graph neighborhood | same graph source with role-specific rendering/filtering |
| `grpc-validate-scope` | `ValidateScope` | allowed and disallowed scope sets | allowed flag and diagnostics |
| `grpc-update-context-basic` | `UpdateContext` | graph-compatible update request | accepted version and warnings behavior |
| `grpc-graph-relationships-basic` | `GetGraphRelationships` | graph query depth within range | root, neighbors, relationships |
| `grpc-graph-relationships-depth-clamp` | `GetGraphRelationships` | requested depth above limit | clamp behavior |

## Required event fixtures

| Fixture ID | Subject | Scenario | Verify |
| --- | --- | --- | --- |
| `nats-graph-node-materialized` | `graph.node.materialized` | valid graph node payload | graph node written to projection path |
| `nats-graph-node-invalid-payload` | `graph.node.materialized` | malformed payload | rejection behavior |
| `nats-node-detail-materialized` | `node.detail.materialized` | valid node detail payload | Valkey detail write path |
| `nats-node-detail-invalid-payload` | `node.detail.materialized` | malformed payload | rejection behavior |

## Required persistence fixtures

| Fixture ID | Surface | Scenario | Verify |
| --- | --- | --- | --- |
| `valkey-node-detail-roundtrip` | node detail store | save and load one node detail | payload equality |
| `snapshot-roundtrip-graph-bundle` | snapshot store | save and load one graph-native bundle | graph bundle equality |
| `neo4j-root-neighborhood` | graph reader | root plus neighbors and relations | full neighborhood reconstruction |

## Fixture data requirements

### Neo4j

- one root node
- several related nodes
- at least two relationship types
- one sparse neighborhood and one dense neighborhood

### Valkey

- extended detail for root node
- extended detail for at least one neighbor
- one node without detail to verify sparse enrichment

## Acceptance set

Before implementation is considered ready, the workspace should pass:

- all gRPC fixtures above
- all event fixtures above
- node detail roundtrip
- snapshot roundtrip for graph-native bundles
- graph neighborhood integration test
