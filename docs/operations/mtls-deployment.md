# mTLS Deployment Guide

How to deploy the rehydration kernel with mutual TLS on all boundaries.

## Prerequisites

- `kubectl` with access to the target cluster
- `helm` 3.x
- cert-manager installed with a working `ClusterIssuer`
- A kernel image with OTLP TLS support (`mtls` tag or later)

## Option A — cert-manager (recommended)

Uses cert-manager to issue and auto-rotate certificates from an internal CA.

### Step 1 — Create internal CA issuer

```bash
NS=underpass-runtime

# Self-signed issuer for bootstrapping the CA
kubectl apply -f - <<EOF
apiVersion: cert-manager.io/v1
kind: Issuer
metadata:
  name: rehydration-kernel-ca-issuer
  namespace: $NS
spec:
  selfSigned: {}
---
apiVersion: cert-manager.io/v1
kind: Certificate
metadata:
  name: rehydration-kernel-internal-ca
  namespace: $NS
spec:
  isCA: true
  commonName: rehydration-kernel-internal-ca
  secretName: rehydration-kernel-internal-ca
  issuerRef:
    name: rehydration-kernel-ca-issuer
    kind: Issuer
  duration: 8760h
---
apiVersion: cert-manager.io/v1
kind: Issuer
metadata:
  name: rehydration-kernel-internal-issuer
  namespace: $NS
spec:
  ca:
    secretName: rehydration-kernel-internal-ca
EOF
```

### Step 2 — Issue per-service certificates

```bash
for svc in grpc nats valkey otel; do
kubectl apply -f - <<EOF
apiVersion: cert-manager.io/v1
kind: Certificate
metadata:
  name: rehydration-kernel-${svc}-cert
  namespace: $NS
spec:
  secretName: rehydration-kernel-${svc}-tls
  issuerRef:
    name: rehydration-kernel-internal-issuer
    kind: Issuer
  commonName: rehydration-kernel-${svc}
  dnsNames:
    - rehydration-kernel
    - rehydration-kernel-${svc}
    - rehydration-kernel.${NS}.svc
    - rehydration-kernel-${svc}.${NS}.svc
    - rehydration-kernel.${NS}.svc.cluster.local
    - rehydration-kernel-${svc}.${NS}.svc.cluster.local
  usages:
    - server auth
    - client auth
  duration: 8760h
EOF
done
```

Each secret contains `tls.crt`, `tls.key`, and `ca.crt` — exactly what
the Helm chart expects. cert-manager auto-rotates before expiry.

### Step 3 — (Optional) Ingress TLS with Let's Encrypt

For external gRPC access with a real certificate:

```bash
kubectl apply -f - <<EOF
apiVersion: cert-manager.io/v1
kind: Certificate
metadata:
  name: rehydration-kernel-tls
  namespace: $NS
spec:
  secretName: rehydration-kernel-tls-prod
  issuerRef:
    name: letsencrypt-prod-r53
    kind: ClusterIssuer
  dnsNames:
    - rehydration-kernel.underpassai.com
EOF
```

Requires a DNS A record pointing to your ingress controller (MetalLB IP):

```bash
aws route53 change-resource-record-sets --hosted-zone-id <ZONE_ID> --change-batch '{
  "Changes": [{"Action":"UPSERT","ResourceRecordSet":{
    "Name":"rehydration-kernel.underpassai.com","Type":"A","TTL":300,
    "ResourceRecords":[{"Value":"<METALLB_IP>"}]
  }}]
}'
```

## Option B — Manual OpenSSL (development / air-gapped)

For environments without cert-manager.

### Step 1 — Generate certificates

```bash
mkdir -p /tmp/kernel-certs && cd /tmp/kernel-certs

openssl req -x509 -newkey rsa:2048 -days 365 -nodes \
  -keyout ca.key -out ca.crt -subj "/CN=rehydration-kernel-ca"

for svc in grpc nats valkey otel; do
  openssl req -newkey rsa:2048 -nodes \
    -keyout ${svc}.key -out ${svc}.csr \
    -subj "/CN=rehydration-kernel-${svc}"
  cat > ${svc}.ext <<EXTEOF
[v3_req]
subjectAltName=DNS:rehydration-kernel-${svc},DNS:rehydration-kernel-${svc}.underpass-runtime.svc,DNS:rehydration-kernel-${svc}.underpass-runtime.svc.cluster.local,DNS:rehydration-kernel
extendedKeyUsage=serverAuth,clientAuth
EXTEOF
  openssl x509 -req -in ${svc}.csr -CA ca.crt -CAkey ca.key -CAcreateserial \
    -out ${svc}.crt -days 365 -extfile ${svc}.ext -extensions v3_req
done
```

### Step 2 — Create Kubernetes secrets

```bash
NS=underpass-runtime
for svc in grpc nats valkey otel; do
  kubectl create secret generic rehydration-kernel-${svc}-tls \
    -n $NS --from-file=tls.crt=${svc}.crt --from-file=tls.key=${svc}.key --from-file=ca.crt=ca.crt
done
```

## Step 3 — Deploy with Helm

```bash
helm upgrade --install rehydration-kernel charts/rehydration-kernel \
  -n underpass-runtime \
  -f charts/rehydration-kernel/values.underpass-runtime.mtls.example.yaml \
  --set image.tag=mtls \
  --timeout 120s
```

The values file configures:

| Boundary | Mode | Secret |
|:---------|:-----|:-------|
| gRPC inbound | mutual TLS | `rehydration-kernel-grpc-tls` |
| Kernel → NATS | mutual TLS | `rehydration-kernel-nats-tls` |
| Kernel → Valkey | mutual TLS (rediss://) | `rehydration-kernel-valkey-tls` |
| Kernel → Neo4j | plaintext in this self-contained profile | in-chart `rehydration-kernel-neo4j` |
| Kernel → OTel Collector | mTLS via env vars | `rehydration-kernel-otel-tls` |
| OTel Collector receiver | mTLS with client CA | `rehydration-kernel-otel-tls` |
| OTel Collector → Loki | mTLS (HTTPS) | `rehydration-kernel-otel-tls` |
| Grafana | anonymous disabled | — |

## Step 4 — Verify

```bash
# Check all pods are running
kubectl -n underpass-runtime get pods | grep rehydration-kernel

# Check kernel logs for TLS initialization
kubectl -n underpass-runtime logs deploy/rehydration-kernel --tail=20

# Check NATS connection is TLS
kubectl -n underpass-runtime logs deploy/rehydration-kernel | grep -i "tls\|connected"

# Check OTel Collector receives data
kubectl -n underpass-runtime logs deploy/rehydration-kernel-otel-collector --tail=10
```

## Troubleshooting

| Symptom | Cause | Fix |
|:--------|:------|:----|
| Kernel pod CrashLoopBackOff | NATS TLS handshake failed | Check SANs match service DNS name |
| `certificate verify failed` | CA mismatch | Ensure all secrets use the same `ca.crt` |
| OTLP export silently fails | Cert paths wrong | Check `OTEL_EXPORTER_OTLP_*_PATH` env vars in pod |
| Valkey connection refused | Wrong scheme | URI must use `rediss://` not `redis://` |
| Neo4j connection failed | Scheme not secure | URI must use `neo4j+s://` not `neo4j://` |

## Neo4j note for this profile

The `values.underpass-runtime.mtls.example.yaml` overlay keeps the self-contained
Neo4j sidecar on plaintext `neo4j://`.

That is intentional:

- the in-chart Neo4j template does not yet expose Bolt TLS
- the kernel can consume TLS-protected Neo4j endpoints
- but that path is currently represented by
  `values.underpass-runtime.secure.example.yaml`, not by the self-contained
  sidecar profile

## What is NOT mTLS yet

- **Neo4j**: The `neo4rs` 0.8 driver only supports CA trust (server verification).
  Client certificate authentication requires a driver upgrade. In addition, the
  in-chart Neo4j sidecar still serves plaintext Bolt today, so a fully secured
  self-contained Neo4j boundary needs chart work as well.

## Certificate Rotation

Certificates in this guide are valid for 365 days. To rotate:

1. Generate new certs with the same CA (or a new CA if compromised)
2. Update the Kubernetes secrets
3. Restart the affected pods: `kubectl rollout restart deploy/rehydration-kernel -n underpass-runtime`

The kernel does not support hot-reload of certificates — a restart is required.
