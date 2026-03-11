## Summary

Describe the change briefly.

## Why This Belongs In The Kernel

Explain why this is kernel-owned work and not integrating-product logic.

## Checks

- [ ] `cargo fmt --all`
- [ ] `cargo check --workspace`
- [ ] `cargo test --workspace`
- [ ] `bash scripts/ci/contract-gate.sh`
- [ ] `bash scripts/ci/quality-gate.sh`

## Contract Impact

- [ ] No public contract changes
- [ ] gRPC contract changed
- [ ] async contract changed
- [ ] runtime integration docs or examples changed

## Architecture Review

- [ ] No god object introduced
- [ ] No god file introduced
- [ ] DDD and hexagonal boundaries preserved
- [ ] No integrating-product nouns added to the kernel boundary
- [ ] Docs updated where needed
