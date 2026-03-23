# Kernel Contract Reference Fixtures

These fixtures are the canonical examples for the generic kernel-owned
integration boundary.

They are split by transport and version:

- `kernel/v1beta1/grpc`: ProtoJSON examples for the canonical
  `underpass.rehydration.kernel.v1beta1` gRPC contract
- `kernel/v1alpha1/grpc`: ProtoJSON examples for the
  `underpass.rehydration.kernel.v1alpha1` gRPC contract kept during transition
- `kernel/v1alpha1/async`: JSON examples for the generic AsyncAPI subjects
- `runtime-reference/v1`: consumer-side runtime examples used by the agentic
  integration reference

They are validated by `rehydration-proto` contract tests and the contract gate:

```bash
bash scripts/ci/contract-gate.sh
```

These fixtures must remain node-centric. Do not introduce product-specific
nouns such as `case_id`, `story_id`, `planning.*`, or `orchestration.*`.

The `runtime-reference/v1` folder is different:

- it is not a kernel-owned public transport contract
- it documents a recommended runtime shape for external consumers
- it should stay generic and agentic, but it is guidance rather than a
  compatibility promise
