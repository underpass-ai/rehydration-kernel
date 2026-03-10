# Proto Contracts

The Rehydration Kernel contracts live under
`api/proto/underpass/rehydration/kernel/v1alpha1`.

Current public surface:

- `ContextQueryService`
- `ContextCommandService`
- `ContextAdminService`

Primary generic contract:

- package: `underpass.rehydration.kernel.v1alpha1`
- anchor identifier: `root_node_id`
- graph-native nouns only: `node`, `relationship`, `detail`

Compatibility note:

- `fleet.context.v1` exists as a migration surface
- it should not be treated as the preferred generic integration contract for a
  new product

Reference:

- [`docs/migration/kernel-node-centric-integration-contract.md`](/home/tirso/ai/developents/rehydration-kernel/docs/migration/kernel-node-centric-integration-contract.md)

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
against product-specific nouns and runs the proto descriptor tests.
