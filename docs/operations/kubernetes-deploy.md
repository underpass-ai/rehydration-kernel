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

That keeps the deploy path aligned with the sibling runtime environment and now
includes the controller-managed gRPC ingress host currently reserved for the
kernel. The kernel chart assumes Neo4j, Valkey, and NATS are required runtime
dependencies; there are no transport disable flags in the deployment contract.

The secure follow-up profile is:

- [`charts/rehydration-kernel/values.underpass-runtime.secure.example.yaml`](../../charts/rehydration-kernel/values.underpass-runtime.secure.example.yaml)

That file captures the intended sibling-runtime target once the shared Neo4j
deployment exposes TLS and the kernel namespace has the matching CA secret.

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

## Smoke Validation

After deploying transport-security changes, use the dedicated smoke script:

- [`scripts/ci/kubernetes-transport-smoke.sh`](../../scripts/ci/kubernetes-transport-smoke.sh)
- runbook: [kubernetes-transport-smoke.md](./kubernetes-transport-smoke.md)

That path validates real in-cluster gRPC TLS and mTLS, and it can also validate
outbound NATS and Valkey TLS against a TLS-enabled environment.

After deploying graph-explorer changes, use the explorer demo workflow:

- [`scripts/demo/run-starship-demo-k8s-job.sh`](../../scripts/demo/run-starship-demo-k8s-job.sh)
- runbook: [graph-explorer-demo.md](./graph-explorer-demo.md)

That path validates the explorer journey itself: root load, node detail,
mid-level zoom, and leaf rehydration against the deployed release.

## gRPC TLS and mTLS

The chart now exposes inbound gRPC transport mode directly:

- `tls.mode=disabled`
- `tls.mode=server`
- `tls.mode=mutual`

When `tls.mode` is `server` or `mutual`, set:

- `tls.existingSecret`
- optionally `tls.mountPath`
- secret keys under `tls.keys.*`

Expected secret data:

- `tls.crt`: server certificate
- `tls.key`: server private key
- `ca.crt`: client CA certificate for `tls.mode=mutual`

Example:

```bash
kubectl create secret generic rehydration-kernel-grpc-tls \
  -n underpass-runtime \
  --from-file=tls.crt=server.crt \
  --from-file=tls.key=server.key \
  --from-file=ca.crt=client-ca.crt
```

Example values override:

```yaml
tls:
  mode: mutual
  existingSecret: rehydration-kernel-grpc-tls
```

## Ingress Exposure

The chart can optionally render a Kubernetes `Ingress` in front of the gRPC
service.

The core values are:

- `ingress.enabled`
- `ingress.className`
- `ingress.annotations`
- `ingress.hosts`
- `ingress.tls`

Example for an NGINX ingress controller serving gRPC:

```yaml
ingress:
  enabled: true
  className: nginx
  annotations:
    nginx.ingress.kubernetes.io/backend-protocol: GRPC
  hosts:
    - host: rehydration-kernel.example.com
      paths:
        - path: /
          pathType: Prefix
  tls:
    - hosts:
        - rehydration-kernel.example.com
      secretName: rehydration-kernel-ingress-tls
```

Controller-specific annotations stay with the operator because gRPC ingress
behavior is controller-specific.

The sibling runtime profile now enables this directly with:

- host: `rehydration-kernel.underpassai.com`
- class: `nginx`
- annotation: `nginx.ingress.kubernetes.io/backend-protocol: GRPC`

## Outbound NATS TLS

The chart now exposes NATS client TLS directly:

- `natsTls.mode=disabled|server|mutual`
- `natsTls.existingSecret`
- `natsTls.mountPath`
- `natsTls.keys.ca`
- `natsTls.keys.cert`
- `natsTls.keys.key`
- `natsTls.tlsFirst`

Expected secret data when using private trust or client identity:

- `ca.crt`: CA certificate for the NATS server
- `tls.crt`: client certificate for `natsTls.mode=mutual`
- `tls.key`: client private key for `natsTls.mode=mutual`

Example:

```bash
kubectl create secret generic rehydration-kernel-nats-tls \
  -n underpass-runtime \
  --from-file=ca.crt=nats-ca.crt \
  --from-file=tls.crt=client.crt \
  --from-file=tls.key=client.key
```

Example values override:

```yaml
natsTls:
  mode: mutual
  existingSecret: rehydration-kernel-nats-tls
  tlsFirst: true
  keys:
    ca: ca.crt
    cert: tls.crt
    key: tls.key
```

`natsTls.mode=server` may also be used without a mounted secret when system
trust roots are sufficient.

## Outbound Valkey TLS

The chart now exposes Valkey TLS material directly:

- `valkeyTls.enabled=true|false`
- `valkeyTls.existingSecret`
- `valkeyTls.mountPath`
- `valkeyTls.keys.ca`
- `valkeyTls.keys.cert`
- `valkeyTls.keys.key`

Expected secret data when using private trust or client identity:

- `ca.crt`: CA certificate for Valkey
- `tls.crt`: client certificate for mutual TLS
- `tls.key`: client private key for mutual TLS

Example:

```bash
kubectl create secret generic rehydration-kernel-valkey-tls \
  -n underpass-runtime \
  --from-file=ca.crt=valkey-ca.crt \
  --from-file=tls.crt=client.crt \
  --from-file=tls.key=client.key
```

Inline connection values are rewritten automatically from `redis://` or
`valkey://` to `rediss://` or `valkeys://`, and Helm appends the configured
`tls_*_path` query parameters using the mounted file paths.

Example values override:

```yaml
valkeyTls:
  enabled: true
  existingSecret: rehydration-kernel-valkey-tls
  keys:
    ca: ca.crt
    cert: tls.crt
    key: tls.key
```

If you use `secrets.existingSecret` for connection URIs, Helm cannot rewrite
the secret-backed values. In that case, store the final `rediss://` or
`valkeys://` URIs in the secret, including `tls_ca_path`, `tls_cert_path`, and
`tls_key_path` query parameters that match the mounted paths.

## Neo4j TLS Today

The Neo4j adapter supports secure schemes such as:

- `bolt+s://`
- `bolt+ssc://`
- `neo4j+s://`
- `neo4j+ssc://`

The chart now supports first-class mounting of a custom Neo4j CA for inline
`connections.graphUri` values.

The shared cluster still serves Neo4j over plaintext `neo4j://` today, so the
default sibling-runtime values stay on the plaintext URI. The staged secure
target lives in
[`charts/rehydration-kernel/values.underpass-runtime.secure.example.yaml`](../../charts/rehydration-kernel/values.underpass-runtime.secure.example.yaml)
and switches the graph connection to `neo4j+s://...` plus `neo4jTls.*`.

Example values override:

```yaml
neo4jTls:
  enabled: true
  existingSecret: rehydration-kernel-neo4j-tls
  keys:
    ca: ca.crt

connections:
  graphUri: bolt+s://neo4j.example.internal:7687
```

That renders `REHYDRATION_GRAPH_URI` with the matching `tls_ca_path` query
parameter that points at the mounted CA file.

Current boundary:

- publicly trusted Neo4j certificates can be handled through the base image
  trust store plus a secure `graphUri`
- private-trust Neo4j deployments can use `neo4jTls.*` with inline
  `connections.graphUri`
- secret-backed `graphUri` values still need the final secure URI, including
  `tls_ca_path`, to be stored in the secret because Helm cannot rewrite secret
  contents
- Neo4j client identity is still outside the current chart surface

## Notes

- `dry_run=true` uses `helm --dry-run=server`
- `atomic=true` is skipped automatically during dry-run
- cluster-specific secrets such as `imagePullSecrets` remain chart/value
  concerns, not workflow concerns
