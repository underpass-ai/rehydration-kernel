# Proto Contracts

The Rehydration Kernel contracts live under
`api/proto/underpass/rehydration/kernel`.

Current public surface:

- `ContextQueryService`
- `ContextCommandService`
- `ContextAdminService`

Primary generic contract:

- package: `underpass.rehydration.kernel.v1beta1`
- anchor identifier: `root_node_id`
- graph-native nouns only: `node`, `relationship`, `detail`

Transitional compatibility contract:

- package: `underpass.rehydration.kernel.v1alpha1`
- kept while downstream clients move to `v1beta1`

Compatibility note:

- `fleet.context.v1` exists as a migration surface
- it should not be treated as the preferred generic integration contract for a
  new product

Reference:

- [`docs/migration/kernel-node-centric-integration-contract.md`](/home/tirso/ai/developents/rehydration-kernel/docs/migration/kernel-node-centric-integration-contract.md)
- [`api/examples/kernel/v1beta1/grpc`](/home/tirso/ai/developents/rehydration-kernel/api/examples/kernel/v1beta1/grpc)
- [`api/examples/kernel/v1alpha1/grpc`](/home/tirso/ai/developents/rehydration-kernel/api/examples/kernel/v1alpha1/grpc)

Validation entrypoints:

```bash
cd api
buf lint

cd ..
bash scripts/ci/contract-gate.sh
```

Rust code generation will be handled in the application build with
`tonic-build`. `buf` is used here to keep the contracts linted and ready for
breaking-change checks. The contract gate also freezes the generic boundary
against product-specific nouns, validates the reference fixtures, and runs the
proto descriptor tests.
