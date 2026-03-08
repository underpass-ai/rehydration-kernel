# Node-Centric Contract Freeze

Status: Draft  
Source of truth: [`CONTEXT_SERVICE_RUST_MIGRATION_PLAN.md`](../../CONTEXT_SERVICE_RUST_MIGRATION_PLAN.md)

## Goal

Freeze the contract and implementation invariants for this repo without
introducing any domain vocabulary that does not exist here.

This repo is node-centric:

- one root node starts rehydration
- related nodes and relationships come from Neo4j
- extended per-node context comes from Valkey
- rendered context is derived from that graph neighborhood

## Phase 0 decisions

1. The internal domain is graph-native and node-centric.
2. The core unit is a node, never a non-graph aggregate.
3. Relationships are first-class data in the bundle.
4. Extended node detail is loaded from Valkey and remains first-class data.
5. Role may affect filtering and rendering, but not the underlying graph shape.
6. No repo documentation or new code should introduce non-graph domain nouns as
   core concepts.

## Frozen gRPC expectations

The repo contract must stay aligned with graph-native rehydration.

### Query expectations

- `GetContext` operates from `root_node_id`
- `RehydrateSession` operates from `root_node_id`
- `ValidateScope` remains role- and scope-based
- `GetGraphRelationships` remains graph-native

### Bundle expectations

The bundle returned by the query path must be graph-native and centered on:

- `root_node_id`
- root node
- neighbor nodes
- relationships
- node details
- bundle metadata
- derived stats

The bundle must not use semantic pack objects as its source of truth.

## Frozen eventing expectations

The repo event model stays node-centric:

- `graph.node.materialized`
- `node.detail.materialized`
- optional bundle-generation notification if needed later

No new implementation work should pivot this repo toward non-graph event
subjects.

## Frozen storage expectations

- Neo4j stores and serves the graph neighborhood
- Valkey stores extended node detail
- snapshot persistence must store graph-native bundle data

## Frozen engineering constraints

- no god objects
- one main concept per file
- explicit boundary mapping
- pure domain
- adapters only at the edges

## Current mismatch to resolve

Today the repo still has a hybrid read path:

- graph-native inputs
- semantic pack-shaped bundle output

That mismatch is the main implementation target for the next slice.

## Acceptance criteria for the freeze

This freeze is valid only while all future implementation work preserves these
rules:

- nodes and relationships remain the core language
- Valkey detail remains node-centric
- any legacy or compatibility naming stays outside the core
