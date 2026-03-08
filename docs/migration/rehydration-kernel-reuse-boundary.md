# Rehydration Kernel Reuse Boundary

This note defines what can be reused from the current workspace while keeping
the repo purely node-centric.

## Internal model invariant

The core model for this repo is:

- one root node
- neighboring nodes
- relationships
- extended node detail from Valkey
- rendering derived from that graph neighborhood

Anything outside that model is adapter detail, not domain truth.

## Crate classification

| Crate / artifact | Classification | Why | Reuse rule |
| --- | --- | --- | --- |
| `crates/rehydration-adapter-neo4j` | reusable with adaptation | graph neighborhood loading is aligned with the target core | reuse graph-query patterns directly |
| `crates/rehydration-adapter-valkey` | reusable with adaptation | node-detail storage is aligned, snapshot shape is not yet | reuse node-detail path directly, adapt snapshot path |
| `crates/rehydration-adapter-nats` | reusable with adaptation | eventing shape is useful, but keep only node-centric subjects | reuse consumer structure |
| `crates/rehydration-application` | reusable with adaptation | layering is good, but current query output is still hybrid | reuse orchestration patterns, refactor bundle flow |
| `crates/rehydration-domain` | reusable with adaptation | graph-side concepts are good, bundle shape is still hybrid | reuse graph concepts, replace bundle model |
| `crates/rehydration-ports` | reusable with adaptation | port-first design is good | keep and rename only when needed |
| `crates/rehydration-testkit` | reusable with adaptation | harness style is useful | rebuild fixtures around graph-native bundles |
| `crates/rehydration-config` | reusable with adaptation | config loading exists | reuse structure only |
| `crates/rehydration-observability` | reusable with adaptation | bootstrap hook exists | extend it without changing domain boundaries |
| `crates/rehydration-proto` | reusable with adaptation | generation plumbing is useful, current bundle shape is not | regenerate after proto refactor |
| `crates/rehydration-transport-grpc` | reusable with adaptation | service plumbing is useful, pack mapping is not | reuse server structure, replace mapping |
| `crates/rehydration-transport-http-admin` | reusable with adaptation | isolated admin transport is fine | keep isolated from the core |
| `crates/rehydration-server` | reusable with adaptation | composition root is useful | keep after refactoring the bundle path |
| `api/proto/underpass/rehydration/kernel/v1alpha1/*` | reusable with adaptation | package and node-centric orientation are fine, bundle shape is not | refactor messages, keep node-centric naming |
| `api/asyncapi/context-projection.v1alpha1.yaml` | reusable with adaptation | node-centric subjects are aligned | keep if still needed after bundle refactor |

## Safe reuse

- crate layout
- port and adapter separation
- graph query adapters
- node detail adapters
- explicit mapping style

## Reuse only after refactor

- current bundle model
- current snapshot serialization
- current gRPC bundle mapping
- current diagnostics derivation

## Do not treat as truth

- any hybrid pack-shaped output
- any synthetic semantic object invented from sections

## Consequence

The next implementation slice should harvest what already works on the
graph-neighborhood side and remove the remaining hybrid semantic layer.
