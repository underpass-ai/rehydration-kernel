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
The kernel also assumes its own infrastructure dependencies are present:
Neo4j, Valkey, and NATS are required runtime components, not optional features.

## Current Status

This repo is functionally complete for the kernel-owned migration scope.

What is already in place:

- graph-native domain and application layers
- split Neo4j, Valkey, NATS, and gRPC adapters
- frozen node-centric gRPC and async contracts
- contract CI with `buf breaking`, AsyncAPI checks, and boundary naming policy
- container-backed integration tests
- full kernel journey end-to-end coverage:
  - projection -> query -> compatibility -> command -> admin
  - full TLS variant across gRPC, NATS, Valkey, and Neo4j in the test harness
- agentic end-to-end proofs:
  - pull-driven runtime flow against a narrow runtime contract shape
  - event-driven runtime trigger flow
- cluster-verifiable starship journey demo for a production-like graph case
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
  - [`api/proto/underpass/rehydration/kernel/v1beta1`](./api/proto/underpass/rehydration/kernel/v1beta1)
- async contract:
  - [`api/asyncapi/context-projection.v1beta1.yaml`](./api/asyncapi/context-projection.v1beta1.yaml)
- contract examples:
  - [`api/examples/README.md`](./api/examples/README.md)
- runtime integration reference:
  - [`docs/migration/kernel-runtime-integration-reference.md`](./docs/migration/kernel-runtime-integration-reference.md)

Historical migration and handoff docs live under [`docs/migration`](./docs/migration).
Many of them are historical after the `v1beta1` cut and compatibility removal.
Use the active integration references first, and treat older migration notes as
review-required unless they explicitly say otherwise.

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

## Paper Artifact

The repository also contains an artifact-backed paper draft on explanatory
graph context rehydration for agentic systems.

Current paper materials:

- submission draft:
  [`docs/PAPER_SUBMISSION_DRAFT.md`](./docs/PAPER_SUBMISSION_DRAFT.md)
- ACL package index:
  [`docs/paper/README.md`](./docs/paper/README.md)
- ACL LaTeX sources:
  [`docs/paper/acl/`](./docs/paper/acl/) (build locally with `pdflatex`)
- paper-use-case artifact summary:
  [`artifacts/paper-use-cases/summary.json`](./artifacts/paper-use-cases/summary.json)
- paper-use-case rendered report:
  [`artifacts/paper-use-cases/results.md`](./artifacts/paper-use-cases/results.md)

The current manuscript evaluates four use cases:

- failure diagnosis with rehydration-point recovery
- why a task was implemented in a particular way
- interrupted handoff with resumable execution
- constraint-preserving retrieval under token pressure

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
bash scripts/ci/integration-agentic-context.sh
bash scripts/ci/integration-agentic-event-context.sh
bash scripts/ci/integration-kernel-full-journey.sh
bash scripts/ci/integration-kernel-full-journey-tls.sh
```

For deployed kernels, the generic projection runtime persists its own state in
Valkey through `REHYDRATION_RUNTIME_STATE_URI`.

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

Public location:

- `ghcr.io/underpass-ai/rehydration-kernel`

Typical pull:

```bash
docker pull ghcr.io/underpass-ai/rehydration-kernel:latest
```

See [`docs/operations/container-image.md`](./docs/operations/container-image.md)
for environment variables, tags, and usage.

Helm chart:

- source chart: [`charts/rehydration-kernel`](./charts/rehydration-kernel)
- OCI location: `oci://ghcr.io/underpass-ai/charts/rehydration-kernel`

The default chart values are intentionally secure:

- no implicit `latest` image tag
- no inline backend URIs by default
- production-style installs should use `image.digest` or a pinned tag plus `secrets.existingSecret`
- optional `ingress.enabled` can expose the gRPC service through a controller-managed ingress
- optional `neo4jTls.*` can mount a custom Neo4j CA for secure `graphUri` values

The sibling-runtime deployment profiles are:

- [`charts/rehydration-kernel/values.underpass-runtime.yaml`](./charts/rehydration-kernel/values.underpass-runtime.yaml) for the current cluster wiring, including the NGINX gRPC ingress host `rehydration-kernel.underpassai.com`
- [`charts/rehydration-kernel/values.underpass-runtime.secure.example.yaml`](./charts/rehydration-kernel/values.underpass-runtime.secure.example.yaml) for the staged Neo4j TLS target once the shared graph service publishes a CA-backed TLS endpoint

For local evaluation only, use [`values.dev.yaml`](./charts/rehydration-kernel/values.dev.yaml).

## License

Apache-2.0. See [`LICENSE`](./LICENSE).
