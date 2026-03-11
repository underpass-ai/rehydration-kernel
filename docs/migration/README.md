# Migration

Migration plans, parity reports, and shadow-mode notes will live here.

Primary roadmap:

- `context-service-rust-roadmap.md`
- `kernel-repo-closeout.md`
- `swe-ai-fleet-node-centric-integration-strategy.md`
- `swe-ai-fleet-shadow-mode-spec.md`
- `swe-ai-fleet-integration-checklist.md`
- `kernel-node-centric-integration-contract.md`
- `kernel-agentic-integration-e2e.md`
- `kernel-agentic-event-trigger-e2e.md`
- `kernel-runtime-integration-reference.md`

Phase 0 baseline:

- `context-service-phase0-contract-freeze.md`
- `context-service-compatibility-matrix.md`
- `context-service-golden-tests.md`
- `context-service-phase2-read-parity-report.md`
- `rehydration-kernel-reuse-boundary.md`

Phase 0 status:

- complete
- `Phase 1 - Compatibility Shell`: complete
- `Phase 2 - Read-Path Parity`: complete
- `Phase 3 - Async NATS Parity`: complete for kernel-owned generic subjects
- kernel contract freeze, contract CI, and reference fixtures: complete
- generic agentic integration proof: complete
- event-driven agentic trigger proof: complete
- runtime integration reference for external consumers: complete
- runnable runtime reference client outside tests: complete
- repo closeout and handoff to integrating products: complete
- shadow mode specification for `swe-ai-fleet`: complete as documentation
- deferred kernel maintenance milestone: consolidate the integration harness
  and reduce CI runtime before the next major growth phase
- next milestone outside the kernel: implement the `swe-ai-fleet` adapter using the node-centric integration strategy and checklist

Historical internal substrate plan:

- `context-service-node-centric-implementation-plan.md`
