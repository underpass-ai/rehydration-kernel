# Graph Explorer Demo

## Purpose

Provide a repeatable operator workflow for proving the graph explorer journey
against a real cluster-backed runtime.

The demo covers:

- root graph load
- multi-hop neighborhood expansion
- zoom into a mid-level node
- `GetNodeDetail`
- leaf rehydration
- rendered context changing when the root changes

## Scripts

- local port-forward journey:
  [`scripts/demo/run-starship-cluster-journey.sh`](../../scripts/demo/run-starship-cluster-journey.sh)
- in-cluster Kubernetes `Job` journey:
  [`scripts/demo/run-starship-demo-k8s-job.sh`](../../scripts/demo/run-starship-demo-k8s-job.sh)

## Requirements

- `kubectl`
- cluster access from the local machine
- a deployed kernel release in `underpass-runtime`
- reachable Neo4j and Valkey backing services

For the local port-forward path:

- `cargo`
- a local Rust toolchain matching the repo toolchain

For the Kubernetes `Job` path:

- the deployed kernel image must contain
  `/usr/local/bin/starship-cluster-journey`

## Local Port-Forward Demo

Use this when you want to run the explorer binary from your workstation while
talking to the cluster through `kubectl port-forward`.

```bash
bash scripts/demo/run-starship-cluster-journey.sh
```

Behavior:

- cleans the seeded starship explorer data
- starts port-forwards for gRPC and NATS
- runs the explorer journey locally
- optionally cleans the seeded data again

Useful variables:

- `KERNEL_NAMESPACE`
- `GRAPH_NAMESPACE`
- `KERNEL_SERVICE`
- `NATS_SERVICE`
- `LOCAL_GRPC_PORT`
- `LOCAL_NATS_PORT`
- `SUBJECT_PREFIX`
- `AUTO_CLEANUP=true|false`

## Kubernetes Job Demo

Use this when you want the explorer journey to run fully in-cluster, without
port-forwarding.

```bash
bash scripts/demo/run-starship-demo-k8s-job.sh
```

Behavior:

- cleans the seeded starship explorer data
- creates a one-shot Kubernetes `Job`
- runs `/usr/local/bin/starship-cluster-journey` inside the cluster
- streams the JSON verification summary to stdout
- optionally deletes the `Job` and seeded data afterwards

Useful variables:

- `KERNEL_NAMESPACE`
- `GRAPH_NAMESPACE`
- `KERNEL_SERVICE`
- `KERNEL_DEPLOYMENT`
- `NATS_SERVICE`
- `JOB_TIMEOUT`
- `JOB_PREFIX`
- `SUBJECT_PREFIX`
- `AUTO_CLEANUP=true|false`

## gRPC TLS and mTLS

The Kubernetes `Job` demo supports gRPC client TLS directly.

Server TLS:

```bash
GRPC_TLS_MODE=server \
GRPC_TLS_SECRET_NAME=rehydration-kernel-grpc-client \
bash scripts/demo/run-starship-demo-k8s-job.sh
```

Mutual TLS:

```bash
GRPC_TLS_MODE=mutual \
GRPC_TLS_SECRET_NAME=rehydration-kernel-grpc-client \
bash scripts/demo/run-starship-demo-k8s-job.sh
```

Expected secret data:

- `ca.crt`
- `client.crt`
- `client.key`

Override key names if your secret uses different keys:

- `GRPC_TLS_CA_KEY`
- `GRPC_TLS_CERT_KEY`
- `GRPC_TLS_KEY_KEY`

The job uses the service DNS name
`${KERNEL_SERVICE}.${KERNEL_NAMESPACE}.svc.cluster.local` as the expected TLS
server name.

## NATS TLS

The explorer job also supports client-side NATS TLS:

- `NATS_TLS_MODE=server|mutual`
- `NATS_TLS_SECRET_NAME`
- `NATS_TLS_FIRST=true|false`
- `NATS_TLS_CA_KEY`
- `NATS_TLS_CERT_KEY`
- `NATS_TLS_KEY_KEY`

This support is primarily for TLS-enabled environments. The shared
`underpass-runtime` environment may still use plaintext NATS.

## Recommended mTLS Validation Path

When you need to validate the explorer job over gRPC mutual TLS without
changing the shared runtime release:

1. Use [`scripts/ci/kubernetes-transport-smoke.sh`](../../scripts/ci/kubernetes-transport-smoke.sh)
   with `grpc-mutual` to create a temporary TLS-enabled release.
2. Point the explorer job to that release with:
   - `KERNEL_SERVICE=<temporary-release>`
   - `KERNEL_DEPLOYMENT=<temporary-release>`
   - `GRPC_TLS_MODE=mutual`
   - `GRPC_TLS_SECRET_NAME=<temporary-release>-grpc-tls`
3. Run `bash scripts/demo/run-starship-demo-k8s-job.sh`.

This is the path used to validate the explorer job's mTLS support when the
graph explorer slice closed.

## Expected Output

Successful runs emit a JSON summary like:

```json
{
  "projection_healthy": true,
  "explorer": {
    "zoom_root": "workstream:containment-control-loop",
    "leaf_detail_loaded": true,
    "leaf_rehydrated": true,
    "rendered_root_changed": true
  }
}
```

The exact token counts may vary, but these conditions should remain true:

- `projection_healthy` is `true`
- `neighbors`, `relationships`, and `details` are non-zero
- `leaf_detail_loaded` is `true`
- `leaf_rehydrated` is `true`
- `rendered_root_changed` is `true`

## Notes

- both demo scripts clean the starship explorer seed by default
- set `AUTO_CLEANUP=false` if you want to inspect the seeded graph after a run
- the Kubernetes `Job` path is the preferred smoke for deployed runtimes
- the port-forward path is the fastest iteration loop when working locally
