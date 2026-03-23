# Kernel Node-Centric Integration Contract

Status: beta canonical, alpha transitional
Scope: kernel-owned public boundary for generic external consumers

## Purpose

Define the public contract that another system may depend on without importing
`swe-ai-fleet` concepts into this repo.

This document is the kernel-side counterpart to:

- [`swe-ai-fleet-node-centric-integration-strategy.md`](./swe-ai-fleet-node-centric-integration-strategy.md)
- [`swe-ai-fleet-integration-checklist.md`](./swe-ai-fleet-integration-checklist.md)

## Contract Boundary

The kernel public boundary is split into two parts:

### 1. Kernel-owned gRPC contract

Package:

- canonical: `underpass.rehydration.kernel.v1beta1`
- transitional: `underpass.rehydration.kernel.v1alpha1`

Services:

- `ContextQueryService`
- `ContextCommandService`
- `ContextAdminService`

Primary identifiers and concepts:

- `root_node_id`
- `node_id`
- `node_kind`
- `relationship_type`
- `BundleNodeDetail`
- `RehydrationBundle`

### 2. Kernel-owned async contract

Generic inbound subjects:

- `graph.node.materialized`
- `node.detail.materialized`

Generic outbound subjects:

- `context.bundle.generated`

These subjects are documented in:

- [`api/asyncapi/context-projection.v1alpha1.yaml`](/home/tirso/ai/developents/rehydration-kernel/api/asyncapi/context-projection.v1alpha1.yaml)

## What External Consumers May Rely On

Consumers integrating directly with the kernel may treat these as stable:

- `root_node_id` is the primary bundle anchor
- graph reads are expressed in terms of nodes and relationships
- extended node context is carried as generic node detail
- render budget and focus are generic request concerns, not domain nouns
- snapshot persistence is generic and not tied to a planning model
- async projection inputs remain node-centric

## What Is Not The Generic Kernel Contract

The following surfaces exist for compatibility or internal purposes, but should
not be treated as the preferred generic integration boundary:

- `fleet.context.v1`
- `context.update.request`
- `context.rehydrate.request`
- `context.update.response`
- `context.rehydrate.response`
- `context.events.updated`

They remain useful for migration, but they are not the target contract for a
new product integrating with the kernel.

## Stability Rules

For `underpass.rehydration.kernel.v1beta1`, the transitional
`underpass.rehydration.kernel.v1alpha1`, and the generic AsyncAPI subjects:

- do not rename services, methods, or field names without an explicit contract
  version change
- do not replace `root_node_id` with product-specific identifiers
- do not add `swe-ai-fleet` nouns to message names or field names
- do not couple async subjects to one product's event vocabulary
- prefer additive changes over semantic rewrites

## Integration Invariants

Any external product integrating directly with the kernel should be able to:

- choose a `root_node_id`
- publish graph materialization and node detail events
- call query or command APIs without adopting kernel-internal storage concerns
- render context around a focused node with bounded token budgets
- persist and retrieve snapshots without importing product-specific write models

If any integration requires this repo to learn product nouns such as
`story`, `task`, `project`, `epic`, `ticket`, or similar, then the integration
belongs in an anti-corruption layer outside the kernel.

## Versioning Guidance

Use `underpass.rehydration.kernel.v1beta1` as the stable boundary for new
integrations while `v1alpha1` remains available only to carry existing clients
through the transition.

Breaking examples:

- renaming `root_node_id`
- changing service names
- changing required semantic meaning of bundle fields
- renaming kernel-owned async subjects

Non-breaking examples:

- adding optional request fields
- adding optional response fields
- adding new generic services or methods
- adding new generic async subjects

## Evidence In Repo

This contract is backed by:

- proto definitions under [`api/proto/underpass/rehydration/kernel/v1beta1`](/home/tirso/ai/developents/rehydration-kernel/api/proto/underpass/rehydration/kernel/v1beta1) and [`api/proto/underpass/rehydration/kernel/v1alpha1`](/home/tirso/ai/developents/rehydration-kernel/api/proto/underpass/rehydration/kernel/v1alpha1)
- async definitions under [`api/asyncapi/context-projection.v1alpha1.yaml`](/home/tirso/ai/developents/rehydration-kernel/api/asyncapi/context-projection.v1alpha1.yaml)
- reference fixtures under [`api/examples/kernel/v1beta1`](/home/tirso/ai/developents/rehydration-kernel/api/examples/kernel/v1beta1) and [`api/examples/kernel/v1alpha1`](/home/tirso/ai/developents/rehydration-kernel/api/examples/kernel/v1alpha1)
- descriptor and contract tests in [`crates/rehydration-proto/src/kernel_v1beta1_contract_tests.rs`](/home/tirso/ai/developents/rehydration-kernel/crates/rehydration-proto/src/kernel_v1beta1_contract_tests.rs), [`crates/rehydration-proto/src/kernel_contract_tests.rs`](/home/tirso/ai/developents/rehydration-kernel/crates/rehydration-proto/src/kernel_contract_tests.rs), and [`crates/rehydration-proto/src/asyncapi_contract_tests.rs`](/home/tirso/ai/developents/rehydration-kernel/crates/rehydration-proto/src/asyncapi_contract_tests.rs)

## Exit Condition

This contract can be treated as integration-ready when:

- another system can build a thin adapter against it
- the adapter does not need kernel changes to express its own domain
- contract drift is caught by tests before merge
