#!/usr/bin/env bash
set -euo pipefail

CHART_PATH="${1:-charts/rehydration-kernel}"
DEV_VALUES="${CHART_PATH}/values.dev.yaml"
UNDERPASS_RUNTIME_VALUES="${CHART_PATH}/values.underpass-runtime.yaml"
UNDERPASS_RUNTIME_MTLS_VALUES="${CHART_PATH}/values.underpass-runtime.mtls.example.yaml"
UNDERPASS_RUNTIME_SECURE_VALUES="${CHART_PATH}/values.underpass-runtime.secure.example.yaml"
DEFAULT_ERR="${TMPDIR:-/tmp}/rehydration-kernel-helm-default.err"

helm lint "${CHART_PATH}" -f "${DEV_VALUES}"
helm template rehydration-kernel "${CHART_PATH}" -f "${DEV_VALUES}" >/tmp/rehydration-kernel-helm-template.yaml

SERVER_TLS_VALUES="${TMPDIR:-/tmp}/rehydration-kernel-helm-server-tls.yaml"
MUTUAL_TLS_VALUES="${TMPDIR:-/tmp}/rehydration-kernel-helm-mutual-tls.yaml"
OUTBOUND_TLS_VALUES="${TMPDIR:-/tmp}/rehydration-kernel-helm-outbound-tls.yaml"
INGRESS_VALUES="${TMPDIR:-/tmp}/rehydration-kernel-helm-ingress.yaml"
NEO4J_TLS_VALUES="${TMPDIR:-/tmp}/rehydration-kernel-helm-neo4j-tls.yaml"
SERVICE_ANNOTATIONS_VALUES="${TMPDIR:-/tmp}/rehydration-kernel-helm-service-annotations.yaml"
PINNED_IMAGE_VALUES="${TMPDIR:-/tmp}/rehydration-kernel-helm-pinned-image.yaml"

cat >"${SERVER_TLS_VALUES}" <<'EOF'
image:
  tag: latest
tls:
  mode: server
  existingSecret: grpc-server-tls
connections:
  graphUri: neo4j://neo4j:7687
  detailUri: redis://valkey:6379
  snapshotUri: redis://valkey:6379
  runtimeStateUri: redis://valkey:6379
  natsUrl: nats://nats:4222
development:
  allowMutableImageTags: true
  allowInlineConnections: true
EOF

cat >"${MUTUAL_TLS_VALUES}" <<'EOF'
image:
  tag: latest
tls:
  mode: mutual
  existingSecret: grpc-mutual-tls
connections:
  graphUri: neo4j://neo4j:7687
  detailUri: redis://valkey:6379
  snapshotUri: redis://valkey:6379
  runtimeStateUri: redis://valkey:6379
  natsUrl: nats://nats:4222
development:
  allowMutableImageTags: true
  allowInlineConnections: true
EOF

helm template rehydration-kernel "${CHART_PATH}" -f "${SERVER_TLS_VALUES}" >/tmp/rehydration-kernel-helm-server-tls-template.yaml
helm template rehydration-kernel "${CHART_PATH}" -f "${MUTUAL_TLS_VALUES}" >/tmp/rehydration-kernel-helm-mutual-tls-template.yaml

cat >"${OUTBOUND_TLS_VALUES}" <<'EOF'
image:
  tag: latest
natsTls:
  mode: mutual
  existingSecret: nats-client-tls
  tlsFirst: true
  keys:
    ca: ca.crt
    cert: tls.crt
    key: tls.key
valkeyTls:
  enabled: true
  existingSecret: valkey-client-tls
  keys:
    ca: ca.crt
    cert: tls.crt
    key: tls.key
connections:
  graphUri: neo4j://neo4j:7687
  detailUri: redis://valkey:6379
  snapshotUri: redis://valkey:6379
  runtimeStateUri: redis://valkey:6379
  natsUrl: nats://nats:4222
development:
  allowMutableImageTags: true
  allowInlineConnections: true
EOF

helm template rehydration-kernel "${CHART_PATH}" -f "${OUTBOUND_TLS_VALUES}" >/tmp/rehydration-kernel-helm-outbound-tls-template.yaml

cat >"${INGRESS_VALUES}" <<'EOF'
image:
  tag: latest
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
connections:
  graphUri: neo4j://neo4j:7687
  detailUri: redis://valkey:6379
  snapshotUri: redis://valkey:6379
  runtimeStateUri: redis://valkey:6379
  natsUrl: nats://nats:4222
development:
  allowMutableImageTags: true
  allowInlineConnections: true
EOF

helm template rehydration-kernel "${CHART_PATH}" -f "${INGRESS_VALUES}" >/tmp/rehydration-kernel-helm-ingress-template.yaml

cat >"${NEO4J_TLS_VALUES}" <<'EOF'
image:
  tag: latest
neo4jTls:
  enabled: true
  existingSecret: neo4j-ca
  keys:
    ca: ca.crt
connections:
  graphUri: bolt+s://neo4j:7687
  detailUri: redis://valkey:6379
  snapshotUri: redis://valkey:6379
  runtimeStateUri: redis://valkey:6379
  natsUrl: nats://nats:4222
development:
  allowMutableImageTags: true
  allowInlineConnections: true
EOF

helm template rehydration-kernel "${CHART_PATH}" -f "${NEO4J_TLS_VALUES}" >/tmp/rehydration-kernel-helm-neo4j-tls-template.yaml

cat >"${SERVICE_ANNOTATIONS_VALUES}" <<'EOF'
image:
  tag: latest
service:
  annotations:
    service.beta.kubernetes.io/aws-load-balancer-scheme: internal
connections:
  graphUri: neo4j://neo4j:7687
  detailUri: redis://valkey:6379
  snapshotUri: redis://valkey:6379
  runtimeStateUri: redis://valkey:6379
  natsUrl: nats://nats:4222
development:
  allowMutableImageTags: true
  allowInlineConnections: true
EOF

helm template rehydration-kernel "${CHART_PATH}" -f "${SERVICE_ANNOTATIONS_VALUES}" >/tmp/rehydration-kernel-helm-service-annotations-template.yaml

cat >"${PINNED_IMAGE_VALUES}" <<'EOF'
image:
  tag: latest
development:
  allowMutableImageTags: true
EOF

helm template rehydration-kernel "${CHART_PATH}" -f "${UNDERPASS_RUNTIME_VALUES}" -f "${PINNED_IMAGE_VALUES}" >/tmp/rehydration-kernel-helm-underpass-runtime-template.yaml
helm template rehydration-kernel "${CHART_PATH}" -f "${UNDERPASS_RUNTIME_MTLS_VALUES}" -f "${PINNED_IMAGE_VALUES}" >/tmp/rehydration-kernel-helm-underpass-runtime-mtls-template.yaml
helm template rehydration-kernel "${CHART_PATH}" -f "${UNDERPASS_RUNTIME_SECURE_VALUES}" -f "${PINNED_IMAGE_VALUES}" >/tmp/rehydration-kernel-helm-underpass-runtime-secure-template.yaml

grep -q "NATS_TLS_MODE" /tmp/rehydration-kernel-helm-outbound-tls-template.yaml
grep -q "NATS_TLS_CERT_PATH" /tmp/rehydration-kernel-helm-outbound-tls-template.yaml
grep -q "rediss://valkey:6379?tls_ca_path=/var/run/rehydration-kernel/valkey-tls/ca.crt&tls_cert_path=/var/run/rehydration-kernel/valkey-tls/tls.crt&tls_key_path=/var/run/rehydration-kernel/valkey-tls/tls.key" /tmp/rehydration-kernel-helm-outbound-tls-template.yaml
grep -q "name: nats-tls" /tmp/rehydration-kernel-helm-outbound-tls-template.yaml
grep -q "name: valkey-tls" /tmp/rehydration-kernel-helm-outbound-tls-template.yaml
grep -q "kind: Ingress" /tmp/rehydration-kernel-helm-ingress-template.yaml
grep -q "nginx.ingress.kubernetes.io/backend-protocol: GRPC" /tmp/rehydration-kernel-helm-ingress-template.yaml
grep -q "host: \"rehydration-kernel.example.com\"" /tmp/rehydration-kernel-helm-ingress-template.yaml
grep -q "bolt+s://neo4j:7687?tls_ca_path=/var/run/rehydration-kernel/neo4j-tls/ca.crt" /tmp/rehydration-kernel-helm-neo4j-tls-template.yaml
grep -q "name: neo4j-tls" /tmp/rehydration-kernel-helm-neo4j-tls-template.yaml
grep -q "service.beta.kubernetes.io/aws-load-balancer-scheme: internal" /tmp/rehydration-kernel-helm-service-annotations-template.yaml
grep -q "host: \"rehydration-kernel.underpassai.com\"" /tmp/rehydration-kernel-helm-underpass-runtime-template.yaml
grep -q "nginx.ingress.kubernetes.io/backend-protocol: GRPC" /tmp/rehydration-kernel-helm-underpass-runtime-template.yaml
grep -q "OTEL_EXPORTER_OTLP_ENDPOINT" /tmp/rehydration-kernel-helm-underpass-runtime-mtls-template.yaml
grep -q "value: \"https://rehydration-kernel-otel-collector:4317\"" /tmp/rehydration-kernel-helm-underpass-runtime-mtls-template.yaml
grep -q "name: otel-tls" /tmp/rehydration-kernel-helm-underpass-runtime-mtls-template.yaml
grep -q "mountPath: \"/var/run/rehydration-kernel/otel-tls\"" /tmp/rehydration-kernel-helm-underpass-runtime-mtls-template.yaml
grep -q "secretName: \"rehydration-kernel-otel-tls\"" /tmp/rehydration-kernel-helm-underpass-runtime-mtls-template.yaml
grep -q "neo4j+s://neo4j:underpassai@neo4j.swe-ai-fleet.svc.cluster.local:7687?tls_ca_path=/var/run/rehydration-kernel/neo4j-tls/ca.crt" /tmp/rehydration-kernel-helm-underpass-runtime-secure-template.yaml
grep -q "secretName: rehydration-kernel-ingress-tls" /tmp/rehydration-kernel-helm-underpass-runtime-secure-template.yaml
grep -q "secretName: \"rehydration-kernel-neo4j-tls\"" /tmp/rehydration-kernel-helm-underpass-runtime-secure-template.yaml

if helm template rehydration-kernel "${CHART_PATH}" > /dev/null 2>"${DEFAULT_ERR}"; then
  echo "default chart render unexpectedly succeeded" >&2
  exit 1
fi

grep -q "set image.tag or image.digest" "${DEFAULT_ERR}"
