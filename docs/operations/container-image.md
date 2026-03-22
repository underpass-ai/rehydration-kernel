# Container Image

## Purpose

Document the standalone kernel image published to GitHub Container Registry.

## Image

Registry:

- `ghcr.io/underpass-ai/rehydration-kernel`

Expected tags:

- `latest` on the default branch
- `main`
- `sha-<short-commit>`
- git tag names such as `v0.1.0` when published from a version tag

## What The Image Contains

The image packages the standalone kernel server binary from
`rehydration-server`.

It is a kernel-owned artifact and is separate from runtime or product images
owned by sibling repos.

## Default Ports

- gRPC: `50054`

## Default Environment In The Image

The image sets container-oriented defaults:

- `REHYDRATION_SERVICE_NAME=rehydration-kernel`
- `REHYDRATION_GRPC_BIND=0.0.0.0:50054`
- `REHYDRATION_ADMIN_BIND=0.0.0.0:8080`
- `REHYDRATION_GRAPH_URI=neo4j://neo4j:7687`
- `REHYDRATION_DETAIL_URI=redis://valkey:6379`
- `REHYDRATION_SNAPSHOT_URI=redis://valkey:6379`
- `REHYDRATION_RUNTIME_STATE_URI=redis://valkey:6379`
- `REHYDRATION_EVENTS_PREFIX=rehydration`
- `ENABLE_NATS=false`
- `ENABLE_PROJECTION_NATS=true`
- `NATS_URL=nats://nats:4222`

`ENABLE_NATS=false` is intentional for standalone evaluation. Integrators can
override it when they want compatibility NATS flows enabled.

`ENABLE_PROJECTION_NATS=true` is the generic kernel default. The projection
runtime persists deduplication markers and checkpoints in Valkey under dedicated
prefixes.

The admin bind setting is carried in config for forward compatibility, but the
standalone image currently exposes only the gRPC port.

## Quick Pull

```bash
docker pull ghcr.io/underpass-ai/rehydration-kernel:latest
```

## Quick Run

Example with externally reachable Neo4j and Valkey:

```bash
docker run --rm \
  -p 50054:50054 \
  -e REHYDRATION_GRAPH_URI=neo4j://host.docker.internal:7687 \
  -e REHYDRATION_DETAIL_URI=redis://host.docker.internal:6379 \
  -e REHYDRATION_SNAPSHOT_URI=redis://host.docker.internal:6379 \
  -e REHYDRATION_RUNTIME_STATE_URI=redis://host.docker.internal:6379 \
  ghcr.io/underpass-ai/rehydration-kernel:latest
```

To enable both the generic projection runtime and compatibility NATS flows:

```bash
docker run --rm \
  -p 50054:50054 \
  -e REHYDRATION_GRAPH_URI=neo4j://host.docker.internal:7687 \
  -e REHYDRATION_DETAIL_URI=redis://host.docker.internal:6379 \
  -e REHYDRATION_SNAPSHOT_URI=redis://host.docker.internal:6379 \
  -e REHYDRATION_RUNTIME_STATE_URI=redis://host.docker.internal:6379 \
  -e ENABLE_NATS=true \
  -e ENABLE_PROJECTION_NATS=true \
  -e NATS_URL=nats://host.docker.internal:4222 \
  ghcr.io/underpass-ai/rehydration-kernel:latest
```

## Local Build

```bash
bash scripts/ci/container-image.sh
```

The script uses `docker` when available and falls back to `podman`. Override
with `CONTAINER_RUNTIME=docker` or `CONTAINER_RUNTIME=podman` to force a
specific runtime.

## Publishing

Publication is handled by:

- [publish-distribution.yml](../../.github/workflows/publish-distribution.yml)

It publishes on:

- push to `main`
- push of tags matching `v*`
- manual workflow dispatch

When the repository cannot push to GHCR with `GITHUB_TOKEN`, set these repo
secrets so the workflow can authenticate with an explicit package writer:

- `GHCR_USERNAME`
- `GHCR_TOKEN`

## Helm Chart

The kernel also ships a standalone Helm chart:

- source: [`charts/rehydration-kernel`](../../charts/rehydration-kernel)
- OCI target: `oci://ghcr.io/underpass-ai/charts/rehydration-kernel`

Security posture of the chart:

- it does not default to `latest`
- it expects `secrets.existingSecret` for backend URIs in non-development installs
- it expects a Valkey-backed `runtimeStateUri` for projection deduplication and checkpoints
- inline `connections.*` are reserved for development-only overrides such as
  [`values.dev.yaml`](../../charts/rehydration-kernel/values.dev.yaml)

Local validation:

```bash
bash scripts/ci/helm-lint.sh
```

Manual deployment via GitHub Actions is documented in:

- [kubernetes-deploy.md](./kubernetes-deploy.md)
