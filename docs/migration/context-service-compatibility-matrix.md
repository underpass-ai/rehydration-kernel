# Node-Centric Compatibility Matrix

This matrix compares the target node-centric architecture for this repo against
the current workspace state.

Status values:

- `match`
- `partial`
- `diverged`
- `missing`

## Core model

| Surface | Target state | Current state | Status | Required action |
| --- | --- | --- | --- | --- |
| Domain vocabulary | node, relationship, node detail, role, bundle metadata | hybrid graph input plus semantic pack output | diverged | remove semantic pack model from the read path |
| Bundle source of truth | graph-native bundle | semantic pack plus pre-rendered sections | diverged | replace `RehydrationBundle` shape |
| Root rehydration unit | `root_node_id` | already `root_node_id` | match | keep |
| Extended context | Valkey-backed per-node detail | already present via node detail store | match | keep |

## Application layer

| Surface | Target state | Current state | Status | Required action |
| --- | --- | --- | --- | --- |
| Bundle reader | `load_bundle()` from graph neighborhood + detail | `load_pack()` from graph neighborhood + semantic collapse | diverged | replace pack reader with bundle reader |
| Rendering | render from nodes, relationships, and node details | render by joining prebuilt sections | diverged | replace renderer |
| Rehydrate session | graph-native bundles per role | semantic-pack bundles per role | diverged | refactor session use case |
| Diagnostics | counts based on nodes, relationships, and details | counts derived from semantic categories | diverged | refactor diagnostics |
| Placeholder assembly | graph-native synthetic bundle | semantic synthetic pack | diverged | replace assembler |

## Transport

| Surface | Target state | Current state | Status | Required action |
| --- | --- | --- | --- | --- |
| Public bundle contract | graph-native bundle | pack-shaped bundle | diverged | update proto and mapping |
| Graph messages | graph node and graph relationship remain first-class | already present in proto | partial | promote them into the main bundle contract |
| Node detail transport | explicit node-detail payload | missing in main bundle contract | missing | add graph-native node detail message |
| gRPC mapping | direct graph-native mapping | mapping depends on semantic pack access | diverged | remove `bundle.pack()` mapping flow |

## Persistence

| Surface | Target state | Current state | Status | Required action |
| --- | --- | --- | --- | --- |
| Neo4j graph read path | graph neighborhood loading | already present | match | keep |
| Valkey node detail | node detail loading | already present | match | keep |
| Snapshot serialization | graph-native bundle snapshot | semantic bundle snapshot | diverged | replace snapshot shape |

## Eventing

| Surface | Target state | Current state | Status | Required action |
| --- | --- | --- | --- | --- |
| Graph node materialization | supported | supported | match | keep |
| Node detail materialization | supported | supported | match | keep |
| Bundle generation notification | optional | optional/internal | partial | only keep if still useful after bundle refactor |

## Immediate consequence

The next implementation slice should not add new domain vocabulary. It should
finish the hard cut from:

- graph input -> semantic pack output

to:

- graph input -> graph-native bundle output
