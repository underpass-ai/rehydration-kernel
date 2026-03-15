# Kubernetes Deploy

## Purpose

Document the manual deployment workflow for the standalone kernel Helm chart.

## Workflow

- workflow: [deploy-kubernetes.yml](../../.github/workflows/deploy-kubernetes.yml)
- trigger: `workflow_dispatch`

The workflow deploys the chart in
[`charts/rehydration-kernel`](../../charts/rehydration-kernel) with
`helm upgrade --install`.

## Required Secret

The workflow expects this repository secret:

- `KUBECONFIG_B64`

The value must be a base64-encoded kubeconfig file.

Example:

```bash
base64 -w0 ~/.kube/config
```

## Inputs

- `namespace`
- `release`
- `values_file`
- `image_tag`
- `image_digest`
- `helm_timeout`
- `dry_run`
- `wait_for_rollout`
- `atomic`

Set either `image_tag` or `image_digest`.

## Default Target

The default values file is:

- [`charts/rehydration-kernel/values.underpass-runtime.yaml`](../../charts/rehydration-kernel/values.underpass-runtime.yaml)

That keeps the deploy path aligned with the sibling runtime environment.

## Local Equivalent

The workflow delegates to:

- [`scripts/ci/deploy-kubernetes.sh`](../../scripts/ci/deploy-kubernetes.sh)

Example:

```bash
RELEASE_NAME=rehydration-kernel \
NAMESPACE=underpass-runtime \
VALUES_FILE=charts/rehydration-kernel/values.underpass-runtime.yaml \
IMAGE_TAG=main \
bash scripts/ci/deploy-kubernetes.sh
```

## Notes

- `dry_run=true` uses `helm --dry-run=server`
- `atomic=true` is skipped automatically during dry-run
- cluster-specific secrets such as `imagePullSecrets` remain chart/value
  concerns, not workflow concerns
