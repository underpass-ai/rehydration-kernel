# Context Service Compatibility Matrix

This matrix compares the external Context Service contract frozen in Phase 0
against the current `main` branch of this repo.

Status values:

- `match`
- `partial`
- `diverged`
- `missing`

## gRPC Identity and Service Layout

| Surface | External source of truth | Current repo state | Status | Required action | Target phase |
| --- | --- | --- | --- | --- | --- |
| package name | `fleet.context.v1` | `underpass.rehydration.kernel.v1alpha1` | diverged | add compatibility proto package and transport shell | Phase 1 |
| service shape | one `ContextService` | three services: query, command, admin | diverged | expose one compatibility service at the edge | Phase 1 |
| read RPC family | `GetContext`, `RehydrateSession`, `ValidateScope`, `GetGraphRelationships` | spread across query and admin services | partial | compose compatibility facade over current use cases | Phase 2 |
| write RPC family | `UpdateContext`, `CreateStory`, `CreateTask`, `AddProjectDecision`, `TransitionPhase` | only `UpdateContext` exists in current public transport | partial | add missing command paths and edge mapping | Phase 4 |

## RPC Coverage

| RPC | External source of truth | Current repo state | Status | Required action | Target phase |
| --- | --- | --- | --- | --- | --- |
| `GetContext` | required | present with different request and response DTOs | partial | build edge request and response mapping | Phase 2 |
| `UpdateContext` | required | present with different field names | partial | preserve external field names and semantics at boundary | Phase 4 |
| `RehydrateSession` | required | present with graph-native response model | partial | render external `packs` response at edge | Phase 2 |
| `ValidateScope` | required | present with different vocabulary and result shape | partial | implement explicit compatibility mapper | Phase 2 |
| `CreateStory` | required | absent | missing | add compatibility command path | Phase 4 |
| `CreateTask` | required | absent | missing | add compatibility command path | Phase 4 |
| `AddProjectDecision` | required | absent | missing | add compatibility command path | Phase 4 |
| `TransitionPhase` | required | absent | missing | add compatibility command path | Phase 4 |
| `GetGraphRelationships` | required | present on admin surface, not compatibility surface | partial | expose on compatibility shell with exact parity rules | Phase 2 |

## Boundary Naming and DTO Shape

| Surface | External source of truth | Current repo state | Status | Required action | Target phase |
| --- | --- | --- | --- | --- | --- |
| primary identifiers | `story_id`, `case_id` | `root_node_id` | diverged | map at transport edge only | Phase 1 |
| task focus identifiers | `task_id`, `subtask_id` | `work_item_id` or no direct equivalent | diverged | keep external names at edge and map internally | Phase 1 |
| update entity field | `entity_type` | `entity_kind` | diverged | preserve external field name | Phase 1 |
| update payload field | `payload` | `payload_json` | diverged | preserve external field name | Phase 1 |
| `GetContextResponse` | `context`, `token_count`, `scopes`, `version`, `blocks` | graph-native bundle and rendered structures | diverged | add compatibility response renderer | Phase 2 |
| `RehydrateSessionResponse` | `case_id`, `generated_at_ms`, `packs`, `stats` | bundle snapshot oriented response | diverged | add compatibility pack renderer | Phase 2 |
| `PromptBlocks` | public message | not exposed | missing | define edge DTO only | Phase 2 |
| `RoleContextPack` | public message | not exposed | missing | define edge DTO only | Phase 2 |

## NATS Contract

| Surface | External source of truth | Current repo state | Status | Required action | Target phase |
| --- | --- | --- | --- | --- | --- |
| planning consumed subjects | six `planning.*` subjects | absent | missing | add compatibility consumers | Phase 3 |
| orchestration consumed subjects | `orchestration.deliberation.completed`, `orchestration.task.dispatched` | absent | missing | add compatibility consumers | Phase 3 |
| async request subjects | `context.update.request`, `context.rehydrate.request` | absent | missing | implement request/reply handlers | Phase 3 |
| async response subjects | `context.update.response`, `context.rehydrate.response` | absent | missing | implement publishers | Phase 3 |
| context-updated event | `context.events.updated` | absent | missing | implement publisher | Phase 4 |
| request wrapper | required `EventEnvelope` | no external envelope parser on the current public path | missing | preserve envelope handling at edge | Phase 3 |
| current internal subjects | not part of external contract | `graph.node.materialized`, `node.detail.materialized` | diverged | keep as internal projection traffic only | Phase 1 |

## Behavioral Parity

| Surface | External source of truth | Current repo state | Status | Required action | Target phase |
| --- | --- | --- | --- | --- | --- |
| `GetGraphRelationships.depth` | clamp to `1..3` | no equivalent clamp on current public path | diverged | implement clamp in compatibility shell | Phase 2 |
| validation error mapping | `INVALID_ARGUMENT` in tested paths | current transport uses internal status mapping | partial | freeze boundary error mapping | Phase 1 |
| unexpected error mapping | `INTERNAL` in tested paths | current transport uses repo-local mapping | partial | align compatibility surface | Phase 1 |
| invalid inbound async JSON | `ack` and drop | no handler | missing | preserve exact behavior | Phase 3 |
| invalid inbound envelope | `ack` and drop | no handler | missing | preserve exact behavior | Phase 3 |
| post-parse handler failure | `nak` | no handler | missing | preserve exact behavior | Phase 3 |

## Configuration and Startup

| Surface | External source of truth | Current repo state | Status | Required action | Target phase |
| --- | --- | --- | --- | --- | --- |
| `GRPC_PORT` default | `50054` | different bootstrap surface | diverged | add compatibility config mapping | Phase 1 |
| `NEO4J_PASSWORD` required | yes | graph access exists, but not under the same public config surface | partial | preserve fail-fast behavior | Phase 1 |
| `REDIS_HOST` and `REDIS_PORT` defaults | `redis`, `6379` | current config differs | diverged | add compatibility env mapping | Phase 1 |
| `NATS_URL` default | `nats://nats:4222` | current config differs | diverged | add compatibility env mapping | Phase 1 |
| `ENABLE_NATS` behavior | parsed as bool; startup fails if false | external NATS service not implemented yet | missing | preserve fail-fast behavior when compatibility shell is enabled | Phase 1 |
| scopes YAML fallback | missing or invalid file -> empty config | current scope path differs | diverged | preserve or document intentional deviation before rollout | Phase 1 |

## Internal Architecture Position

| Surface | External source of truth | Current repo state | Status | Required action | Target phase |
| --- | --- | --- | --- | --- | --- |
| internal domain language | unspecified by external contract | node-centric core | match | keep | all phases |
| location of legacy vocabulary | boundary concern only | not implemented yet | missing | keep external nouns confined to adapters and transport | Phase 1 |

## Closeout

Phase 0 shows that this repo is ahead on internal architecture and behind on
external compatibility.

The next slice is therefore not another internal refactor.
The next slice is the compatibility shell for:

- `fleet.context.v1`
- external NATS subjects and envelopes
- external status mapping and config behavior
