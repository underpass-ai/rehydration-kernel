# Kernel Contract Reference Fixtures

These fixtures are the canonical examples for the generic kernel-owned
integration boundary.

They are split by transport:

- `kernel/v1alpha1/grpc`: ProtoJSON examples for the
  `underpass.rehydration.kernel.v1alpha1` gRPC contract
- `kernel/v1alpha1/async`: JSON examples for the generic AsyncAPI subjects

They are validated by `rehydration-proto` contract tests and the contract gate:

```bash
bash scripts/ci/contract-gate.sh
```

These fixtures must remain node-centric. Do not introduce product-specific
nouns such as `case_id`, `story_id`, `planning.*`, or `orchestration.*`.
