# Proto Contracts

The Rehydration Kernel contracts live under
`api/proto/underpass/rehydration/kernel/v1alpha1`.

Current public surface:

- `ContextQueryService`
- `ContextAdminService`

Validation entrypoints:

```bash
cd api
buf lint
```

Rust code generation will be handled in the application build with
`tonic-build`. `buf` is used here to keep the contracts linted and ready for
breaking-change checks.
