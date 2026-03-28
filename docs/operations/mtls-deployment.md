# mTLS Deployment Guide

How to deploy the rehydration kernel with mutual TLS on all boundaries.

## Prerequisites

- `kubectl` with access to the target cluster
- `openssl` for certificate generation
- `helm` 3.x
- A kernel image with OTLP TLS support (`quality-observers` tag or later)

## Step 1 — Generate certificates

Create a shared CA and per-service certificates. All services use the same CA
so they can mutually verify each other.

```bash
mkdir -p /tmp/kernel-certs && cd /tmp/kernel-certs

# CA
openssl req -x509 -newkey rsa:2048 -days 365 -nodes \
  -keyout ca.key -out ca.crt \
  -subj "/CN=rehydration-kernel-ca"

# Per-service certs (gRPC, NATS, Valkey, OTel)
for svc in grpc nats valkey otel; do
  openssl req -newkey rsa:2048 -nodes \
    -keyout ${svc}.key -out ${svc}.csr \
    -subj "/CN=rehydration-kernel-${svc}"

  cat > ${svc}.ext <<EOF
[v3_req]
subjectAltName=DNS:rehydration-kernel-${svc},DNS:rehydration-kernel-${svc}.underpass-runtime.svc,DNS:rehydration-kernel-${svc}.underpass-runtime.svc.cluster.local,DNS:rehydration-kernel
extendedKeyUsage=serverAuth,clientAuth
EOF

  openssl x509 -req -in ${svc}.csr -CA ca.crt -CAkey ca.key -CAcreateserial \
    -out ${svc}.crt -days 365 -extfile ${svc}.ext -extensions v3_req
done
```

Each cert has SANs for the Kubernetes service DNS names and allows both
`serverAuth` and `clientAuth` (required for mTLS).

## Step 2 — Create Kubernetes secrets

```bash
NS=underpass-runtime

# gRPC (server cert + CA for client verification)
kubectl create secret generic rehydration-kernel-grpc-tls \
  -n $NS --from-file=tls.crt=grpc.crt --from-file=tls.key=grpc.key --from-file=ca.crt=ca.crt

# NATS (server cert + CA)
kubectl create secret generic rehydration-kernel-nats-tls \
  -n $NS --from-file=tls.crt=nats.crt --from-file=tls.key=nats.key --from-file=ca.crt=ca.crt
kubectl create secret generic rehydration-kernel-nats-ca \
  -n $NS --from-file=ca.crt=ca.crt

# Valkey (server cert + CA)
kubectl create secret generic rehydration-kernel-valkey-tls \
  -n $NS --from-file=tls.crt=valkey.crt --from-file=tls.key=valkey.key --from-file=ca.crt=ca.crt
kubectl create secret generic rehydration-kernel-valkey-ca \
  -n $NS --from-file=ca.crt=ca.crt

# Neo4j (CA only — mTLS pending driver upgrade)
kubectl create secret generic rehydration-kernel-neo4j-tls \
  -n $NS --from-file=ca.crt=ca.crt

# OTel Collector (server cert + CA)
kubectl create secret generic rehydration-kernel-otel-tls \
  -n $NS --from-file=tls.crt=otel.crt --from-file=tls.key=otel.key --from-file=ca.crt=ca.crt
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
| Kernel → Neo4j | TLS with CA trust | `rehydration-kernel-neo4j-tls` |
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

## What is NOT mTLS yet

- **Neo4j**: The `neo4rs` 0.8 driver only supports CA trust (server verification).
  Client certificate authentication requires a driver upgrade. The URI parsing
  and Helm values are ready — only the driver call is missing.

## Certificate Rotation

Certificates in this guide are valid for 365 days. To rotate:

1. Generate new certs with the same CA (or a new CA if compromised)
2. Update the Kubernetes secrets
3. Restart the affected pods: `kubectl rollout restart deploy/rehydration-kernel -n underpass-runtime`

The kernel does not support hot-reload of certificates — a restart is required.
