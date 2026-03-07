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
- `crates/rehydration-application`: use cases and orchestration.
- `crates/rehydration-proto`: generated protobuf and gRPC stubs.
- `crates/rehydration-transport-grpc`: public query transport.
- `crates/rehydration-transport-http-admin`: admin transport placeholder.
- `crates/rehydration-adapter-*`: infrastructure adapters.
- `crates/rehydration-server`: composition root.
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

## Quickstart

```bash
cargo fmt --all
cargo check --workspace
cargo test --workspace
```

```bash
scripts/ci/quality-gate.sh
```

## SonarCloud

The GitHub Actions CI includes a `sonarcloud` job wired for Rust LCOV coverage.
It is configured for:

- organization `underpass-ai-swe-ai-fleet`
- project key `underpass-ai_rehydration-kernel`

To enable the scan, configure:

- repository secret `SONAR_TOKEN`

If the secret is absent, the job exits cleanly with a skip notice instead of
failing the whole pipeline.
