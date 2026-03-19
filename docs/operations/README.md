# Operations

Operational docs for the kernel live here.

Current contents:

- [`deployment-boundary.md`](./deployment-boundary.md)
- [`container-image.md`](./container-image.md)
- [`kubernetes-deploy.md`](./kubernetes-deploy.md)
- [`kubernetes-transport-smoke.md`](./kubernetes-transport-smoke.md)
- [`graph-explorer-demo.md`](./graph-explorer-demo.md)

Key rule:

- this repo owns kernel operations and contract documentation
- this repo owns the standalone kernel image artifact
- this repo owns the standalone kernel Helm chart
- sibling repos may own runnable deployment packaging such as GHCR images,
  Docker Compose bundles, or Helm charts when those artifacts package a runtime
  or product layer rather than the kernel itself
