# Helm Installation Guide — rehydration-kernel

## Overview

The kernel chart deploys the rehydration-kernel gRPC service with optional
self-contained infrastructure: Neo4j, NATS, Valkey, OTel Collector, Loki,
Grafana. It owns the shared CA for the entire namespace.

**Deploy order**: kernel first (creates CA), then runtime, then demo.

## Quick Start — Full mTLS with cert-gen

```bash
# One command — cert-gen creates CA + all server/client certs automatically
helm upgrade --install rehydration-kernel charts/rehydration-kernel \
  -n underpass-runtime --create-namespace \
  --set image.tag=dev-6d98dfe \
  --set certGen.enabled=true \
  -f charts/rehydration-kernel/values.underpass-runtime.yaml \
  -f charts/rehydration-kernel/values.underpass-runtime.mtls.example.yaml
```

This creates 7 secrets from the shared CA:

| Secret | Purpose |
|--------|---------|
| `rehydration-kernel-internal-ca` | ECDSA P-256 CA (10-year validity) |
| `rehydration-kernel-grpc-tls` | Kernel gRPC server cert |
| `rehydration-kernel-nats-tls` | NATS server cert |
| `rehydration-kernel-valkey-tls` | Valkey server cert |
| `rehydration-kernel-nats-ca` | CA mirror for NATS config |
| `rehydration-kernel-valkey-ca` | CA mirror for Valkey config |
| `rehydration-kernel-otel-tls` | OTel Collector server cert |
| `rehydration-kernel-client-tls` | Kernel client cert (NATS/Valkey mTLS) |

## Values Profiles

| File | Use Case |
|------|----------|
| `values.yaml` | Base defaults (all disabled) |
| `values.underpass-runtime.yaml` | Self-contained: Neo4j + NATS + Valkey + observability |
| `values.underpass-runtime.mtls.example.yaml` | Full mTLS overlay |
| `values.dev.yaml` | Local development |
| `values.shared-infra.yaml` | Use existing NATS/Valkey (no sidecars) |

## Real Examples

### Development (no TLS, local cluster)

```bash
helm upgrade --install rehydration-kernel charts/rehydration-kernel \
  -n underpass-runtime --create-namespace \
  --set image.tag=dev-latest \
  -f charts/rehydration-kernel/values.dev.yaml
```

### Self-contained with full mTLS (current cluster state)

```bash
helm upgrade --install rehydration-kernel charts/rehydration-kernel \
  -n underpass-runtime \
  --set image.tag=dev-6d98dfe \
  --set certGen.enabled=true \
  --set certGen.image=ghcr.io/underpass-ai/underpass-runtime/cert-gen:v1.0.0 \
  -f charts/rehydration-kernel/values.underpass-runtime.yaml \
  -f charts/rehydration-kernel/values.underpass-runtime.mtls.example.yaml
```

### Verify deployment

```bash
# All pods running
kubectl get pods -n underpass-runtime -l app.kubernetes.io/instance=rehydration-kernel

# Kernel gRPC healthy
kubectl logs -n underpass-runtime -l app.kubernetes.io/name=rehydration-kernel --tail=5

# Certs generated
kubectl get secrets -n underpass-runtime | grep rehydration-kernel

# Verify cert chain
kubectl get secret rehydration-kernel-grpc-tls -n underpass-runtime \
  -o jsonpath='{.data.tls\.crt}' | base64 -d | \
  openssl verify -CAfile <(kubectl get secret rehydration-kernel-internal-ca \
  -n underpass-runtime -o jsonpath='{.data.tls\.crt}' | base64 -d)
```

### Certificate rotation

```bash
# Delete the cert to rotate, then upgrade
kubectl delete secret rehydration-kernel-grpc-tls -n underpass-runtime
helm upgrade rehydration-kernel charts/rehydration-kernel \
  -n underpass-runtime --reuse-values
# cert-gen Job regenerates only the missing secret
```

## Services Deployed

| Service | Port | Protocol |
|---------|------|----------|
| `rehydration-kernel` | 50054 | gRPC (mTLS) |
| `rehydration-kernel-nats` | 4222 | NATS (mTLS) |
| `rehydration-kernel-valkey` | 6379 | Redis (mTLS) |
| `rehydration-kernel-neo4j` | 7687/7474 | Bolt/HTTP (plaintext) |
| `rehydration-kernel-otel-collector` | 4317/9090 | OTLP/Prometheus |
| `rehydration-kernel-loki` | 3100 | HTTP |
| `rehydration-kernel-grafana` | 3000 | HTTP |

## certGen Configuration

```yaml
certGen:
  enabled: false                    # Enable cert-gen hook Job
  image: ghcr.io/underpass-ai/underpass-runtime/cert-gen:v1.0.0
  keyCurve: prime256v1              # ECDSA curve
  validityDays: 365                 # Cert validity
  caValidityDays: 3650              # CA validity (10 years)
  caCommonName: rehydration-kernel-internal-ca
```
