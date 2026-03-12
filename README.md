# Rehydration Kernel

Node-centric context rehydration for agentic systems.

## What This Repo Is

`rehydration-kernel` is a generic context engine built around four public
concepts:

- root node
- neighbor nodes
- relationships
- extended node detail

The kernel does not own product-specific nouns. Integrating products are
expected to map their own domain language to this graph model at the edge.

## Current Status

This repo is functionally complete for the kernel-owned migration scope.

What is already in place:

- graph-native domain and application layers
- split Neo4j, Valkey, NATS, and gRPC adapters
- frozen node-centric gRPC and async contracts
- contract CI with `buf breaking`, AsyncAPI checks, and boundary naming policy
- container-backed integration tests
- agentic end-to-end proofs:
  - pull-driven runtime flow
  - event-driven runtime trigger flow
- runtime integration reference docs and runnable client example

What is intentionally out of scope for this repo:

- `swe-ai-fleet` legacy noun modeling
- `planning.*` or `orchestration.*` consumers
- product-side shadow mode implementation
- rollout and rollback logic

## Architecture

Non-negotiable repo rules:

- DDD first
- hexagonal boundaries
- no god objects
- no god files
- one main concept per file
- one use case per file

Internal core language:

- root node
- neighbor nodes
- relationships
- node details in Valkey

## Contracts

Primary public artifacts:

- gRPC proto:
  - [`api/proto/underpass/rehydration/kernel/v1alpha1`](./api/proto/underpass/rehydration/kernel/v1alpha1)
- async contract:
  - [`api/asyncapi/context-projection.v1alpha1.yaml`](./api/asyncapi/context-projection.v1alpha1.yaml)
- contract examples:
  - [`api/examples/README.md`](./api/examples/README.md)
- runtime integration reference:
  - [`docs/migration/kernel-runtime-integration-reference.md`](./docs/migration/kernel-runtime-integration-reference.md)

Historical migration and handoff docs live under [`docs/migration`](./docs/migration).
They are useful for adopters, but they do not redefine the kernel domain.

## Repo Layout

- `api/proto`: gRPC contracts
- `api/asyncapi`: async contracts
- `api/examples`: canonical request, response, and event fixtures
- `crates/rehydration-domain`: domain model and invariants
- `crates/rehydration-ports`: application-facing ports
- `crates/rehydration-application`: use cases and orchestration
- `crates/rehydration-adapter-*`: infrastructure adapters
- `crates/rehydration-transport-*`: transport boundaries
- `crates/rehydration-server`: composition root
- `crates/rehydration-testkit`: testing helpers
- `scripts/ci`: local quality and integration gates
- `docs/migration`: closeout, handoff, and integration strategy docs

## Quickstart

Toolchain:

- Rust `1.90.0`, pinned in [`rust-toolchain.toml`](./rust-toolchain.toml)

Core checks:

```bash
cargo fmt --all
cargo check --workspace
cargo test --workspace
```

Repository gate:

```bash
bash scripts/ci/quality-gate.sh
```

Focused contract gate:

```bash
bash scripts/ci/contract-gate.sh
```

Container-backed integration targets:

```bash
bash scripts/ci/integration-valkey.sh
bash scripts/ci/integration-neo4j.sh
bash scripts/ci/integration-nats-compatibility.sh
bash scripts/ci/integration-grpc-compatibility.sh
bash scripts/ci/integration-agentic-context.sh
bash scripts/ci/integration-agentic-event-context.sh
```

For deployed kernels, the generic projection runtime is enabled separately from
legacy compatibility NATS and persists its own state in Valkey through
`REHYDRATION_RUNTIME_STATE_URI`.

Container image build check:

```bash
bash scripts/ci/container-image.sh
```

The script uses `docker` when available and falls back to `podman`. Override
with `CONTAINER_RUNTIME=docker` or `CONTAINER_RUNTIME=podman` if you need to
force one runtime.

Helm chart lint:

```bash
bash scripts/ci/helm-lint.sh
```

## Public Readiness Notes

If you are integrating another product with this kernel, start here:

- [`docs/migration/kernel-node-centric-integration-contract.md`](./docs/migration/kernel-node-centric-integration-contract.md)
- [`docs/migration/kernel-runtime-integration-reference.md`](./docs/migration/kernel-runtime-integration-reference.md)
- [`docs/migration/kernel-repo-closeout.md`](./docs/migration/kernel-repo-closeout.md)

If you are integrating `swe-ai-fleet`, the handoff docs are:

- [`docs/migration/swe-ai-fleet-node-centric-integration-strategy.md`](./docs/migration/swe-ai-fleet-node-centric-integration-strategy.md)
- [`docs/migration/swe-ai-fleet-shadow-mode-spec.md`](./docs/migration/swe-ai-fleet-shadow-mode-spec.md)
- [`docs/migration/swe-ai-fleet-integration-checklist.md`](./docs/migration/swe-ai-fleet-integration-checklist.md)

## Runtime And Deployment Ecosystem

This repo owns the kernel code, contracts, and integration proofs.

Operational packaging may live in sibling repos that consume the kernel. In the
current ecosystem that includes a sibling runtime capable of:

- publishing container artifacts to GitHub Container Registry
- running under Docker Compose
- running on Kubernetes through Helm

That deployment packaging is intentionally kept outside the kernel repo when it
belongs to the runtime or product layer rather than to the kernel itself.

See:

- [`docs/operations/README.md`](./docs/operations/README.md)
- [`docs/operations/deployment-boundary.md`](./docs/operations/deployment-boundary.md)
- [`docs/operations/container-image.md`](./docs/operations/container-image.md)

## Standalone Container Image

The kernel now owns a standalone OCI image intended for external download and
evaluation.

Planned public location:

- `ghcr.io/underpass-ai/rehydration-kernel`

Typical pull:

```bash
docker pull ghcr.io/underpass-ai/rehydration-kernel:latest
```

See [`docs/operations/container-image.md`](./docs/operations/container-image.md)
for environment variables, tags, and usage.

Helm chart:

- source chart: [`charts/rehydration-kernel`](./charts/rehydration-kernel)
- planned OCI location: `oci://ghcr.io/underpass-ai/charts/rehydration-kernel`

The default chart values are intentionally secure:

- no implicit `latest` image tag
- no inline backend URIs by default
- production-style installs should use `image.digest` or a pinned tag plus `secrets.existingSecret`

For local evaluation only, use [`values.dev.yaml`](./charts/rehydration-kernel/values.dev.yaml).

## License

Apache-2.0. See [`LICENSE`](./LICENSE).
