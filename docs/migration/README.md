# Migration

> Historical migration area. After the `v1beta1` cut and compatibility removal,
> several files in this directory are obsolete or require detailed review
> before reuse. Do not treat this folder as the primary source of truth for the
> current kernel contract.

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
- `llm-response-determinism-strategy.md`
- `transport-security-v1.md`

Phase 0 status:

- complete
- kernel contract freeze, contract CI, and reference fixtures: complete
- generic agentic integration proof: complete
- event-driven agentic trigger proof: complete
- runtime integration reference for external consumers: complete
- runnable runtime reference client outside tests: complete
- LLM response determinism strategy: planned and documented
- transport security v1: implemented for gRPC, outbound NATS, outbound Valkey, and Neo4j CA wiring; Neo4j client identity remains open
- repo closeout and handoff to integrating products: complete
- shadow mode specification for `swe-ai-fleet`: complete as documentation
- deferred kernel maintenance milestone: consolidate the integration harness
  and reduce CI runtime before the next major growth phase
- next milestone outside the kernel: implement the `swe-ai-fleet` adapter using the node-centric integration strategy and checklist

Historical internal substrate plan:

- `context-service-node-centric-implementation-plan.md`
