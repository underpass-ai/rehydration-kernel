# Rehydration Kernel

Deterministic context rehydration for agentic systems.

## Status

Initial Rust workspace scaffold for the extraction of the context rehydration
capability into an independent repository.

## Workspace

- `api/proto`: gRPC contracts.
- `api/asyncapi`: event contracts.
- `crates/rehydration-domain`: domain model and invariants.
- `crates/rehydration-ports`: stable application-facing traits.
- `crates/rehydration-application`: use cases, query/admin/command
  orchestration, and application DTOs.
- `crates/rehydration-proto`: generated protobuf and gRPC stubs.
- `crates/rehydration-transport-grpc`: tonic gRPC transport for query, command,
  and admin services.
- `crates/rehydration-transport-http-admin`: admin transport placeholder.
- `crates/rehydration-adapter-*`: infrastructure adapters.
- `crates/rehydration-server`: composition root and async tonic bootstrap.
- `crates/rehydration-testkit`: in-memory testing helpers.

## Toolchain

The repo is pinned to Rust `1.90.0` through `rust-toolchain.toml`.

## API Contracts

The first API split is defined under
`api/proto/underpass/rehydration/kernel/v1alpha1` with:

- `ContextQueryService`
- `ContextCommandService`
- `ContextAdminService`

Rust stubs are generated at build time by `tonic-build` inside
`crates/rehydration-proto`.

The server bootstrap currently exposes all three gRPC services through `tonic`.
Query, admin, and command flows are already mediated by dedicated application
services, while the core domain logic behind those flows is still being ported.
The Valkey snapshot adapter now writes real RESP `SET` commands over TCP with a
stable JSON payload, and the Neo4j adapter has been hardened to avoid
manufacturing synthetic bundles from infrastructure. The first real Neo4j read
path now loads `RoleContextPackProjection` roots linked to `CaseHeader`,
`PlanHeader`, `WorkItem`, `Decision`, `DecisionRelation`, `TaskImpact`, and
`Milestone` projection nodes.

The event-driven projection foundation is node-centric by design. Inbound
projection events no longer model `project`, `epic`, `story`, or `task`
concepts directly; they model:

- graph nodes plus their relations;
- expanded per-node detail materialized into Valkey by `node_id`;
- deterministic idempotency and per-consumer checkpoints.

The first real projection write path is now split the same way:

- Neo4j persists normalized graph nodes and node-to-node relations.
- Valkey persists expanded node detail keyed by `node_id`.
- `RoutingProjectionWriter` fans projection mutations out to those stores.

The current gRPC query path still reads the earlier `RoleContextPackProjection`
model. Migrating query assembly to the node-centric graph is the next step; the
new write path is in place without pretending that migration is already done.

## Quickstart

```bash
cargo fmt --all
cargo check --workspace
cargo test --workspace
```

```bash
scripts/ci/quality-gate.sh
```

```bash
bash scripts/ci/integration-valkey.sh
```

```bash
CONTAINER_RUNTIME=docker bash scripts/ci/integration-valkey.sh
CONTAINER_RUNTIME=podman bash scripts/ci/integration-valkey.sh
CONTAINER_RUNTIME=docker bash scripts/ci/integration-neo4j.sh
CONTAINER_RUNTIME=podman bash scripts/ci/integration-neo4j.sh
```

The container-backed integration targets use `testcontainers` and are intentionally
separated from `cargo test --workspace` so unit checks stay fast and
container-backed tests remain explicit.

Container runtime bootstrap for local integration tests lives in
`scripts/ci/testcontainers-runtime.sh`, so individual integration scripts only
need to source that setup and run their target.

Local runtime selection works like this:

- `auto`: prefer `Docker`; if it is unavailable, fall back to `Podman`.
- `docker`: require a working Docker daemon.
- `podman`: use a Docker-compatible Podman socket, first from the standard user
  socket, then by trying `podman.socket`, and finally by launching a temporary
  `podman system service`. In this mode the script exports
  `TESTCONTAINERS_RYUK_DISABLED=true`.

GitHub Actions stays on Docker for the repository CI path.

For Neo4j-backed local runs, `REHYDRATION_GRAPH_URI` may include credentials,
for example `neo4j://neo4j:<password>@localhost:7687`.

## SonarCloud

The GitHub Actions CI includes a `sonarcloud` job wired for Rust LCOV coverage.
It is configured for:

- organization `underpass-ai-swe-ai-fleet`
- project key `underpass-ai_rehydration-kernel`

To enable the scan, configure:

- repository secret `SONAR_TOKEN`

If the secret is absent, the job exits cleanly with a skip notice instead of
failing the whole pipeline.
