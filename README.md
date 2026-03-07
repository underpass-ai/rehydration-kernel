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
- `crates/rehydration-transport-grpc`: public query transport.
- `crates/rehydration-transport-http-admin`: admin transport placeholder.
- `crates/rehydration-adapter-*`: infrastructure adapters.
- `crates/rehydration-server`: composition root.
- `crates/rehydration-testkit`: in-memory testing helpers.

## Toolchain

The repo is pinned to Rust `1.90.0` through `rust-toolchain.toml`.

## Quickstart

```bash
cargo fmt --all
cargo check --workspace
cargo test --workspace
```
