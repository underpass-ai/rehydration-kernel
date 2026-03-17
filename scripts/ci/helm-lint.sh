#!/usr/bin/env bash
set -euo pipefail

CHART_PATH="${1:-charts/rehydration-kernel}"
DEV_VALUES="${CHART_PATH}/values.dev.yaml"
DEFAULT_ERR="${TMPDIR:-/tmp}/rehydration-kernel-helm-default.err"

helm lint "${CHART_PATH}" -f "${DEV_VALUES}"
helm template rehydration-kernel "${CHART_PATH}" -f "${DEV_VALUES}" >/tmp/rehydration-kernel-helm-template.yaml

SERVER_TLS_VALUES="${TMPDIR:-/tmp}/rehydration-kernel-helm-server-tls.yaml"
MUTUAL_TLS_VALUES="${TMPDIR:-/tmp}/rehydration-kernel-helm-mutual-tls.yaml"
OUTBOUND_TLS_VALUES="${TMPDIR:-/tmp}/rehydration-kernel-helm-outbound-tls.yaml"

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

grep -q "NATS_TLS_MODE" /tmp/rehydration-kernel-helm-outbound-tls-template.yaml
grep -q "NATS_TLS_CERT_PATH" /tmp/rehydration-kernel-helm-outbound-tls-template.yaml
grep -q "rediss://valkey:6379?tls_ca_path=/var/run/rehydration-kernel/valkey-tls/ca.crt&tls_cert_path=/var/run/rehydration-kernel/valkey-tls/tls.crt&tls_key_path=/var/run/rehydration-kernel/valkey-tls/tls.key" /tmp/rehydration-kernel-helm-outbound-tls-template.yaml
grep -q "name: nats-tls" /tmp/rehydration-kernel-helm-outbound-tls-template.yaml
grep -q "name: valkey-tls" /tmp/rehydration-kernel-helm-outbound-tls-template.yaml

if helm template rehydration-kernel "${CHART_PATH}" > /dev/null 2>"${DEFAULT_ERR}"; then
  echo "default chart render unexpectedly succeeded" >&2
  exit 1
fi

grep -q "set image.tag or image.digest" "${DEFAULT_ERR}"
