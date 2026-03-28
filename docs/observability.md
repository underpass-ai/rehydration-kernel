# Observability

The kernel emits quality metrics through a hexagonal observer port with pluggable
backends. Every successful `GetContext` and `GetContextPath` render produces metrics
that flow through both OTel (Prometheus/Grafana) and structured logs (Loki/Grafana).

Architecture reference: [ADR-007](adr/ADR-007-quality-metrics-observability.md)

## BundleQualityMetrics

A domain value object computed on every render. Invariant-validated at construction.

| Metric | Type | Range | Description |
|:-------|:-----|:------|:------------|
| `raw_equivalent_tokens` | u32 | >= 0 | Token count for a flat text dump of the same data |
| `compression_ratio` | f64 | >= 0.0 | `raw_equivalent_tokens / rendered_tokens`. >1.0 = compression |
| `causal_density` | f64 | [0.0, 1.0] | Fraction of causal/motivational/evidential relationships |
| `noise_ratio` | f64 | [0.0, 1.0] | Fraction of noise/distractor nodes |
| `detail_coverage` | f64 | [0.0, 1.0] | Fraction of nodes with extended detail |

Computed by `BundleQualityMetrics::compute(bundle, rendered_tokens, estimator)` in the
domain layer. The testkit provides `seed_to_bundle()` to convert generated seeds into
domain bundles, and `seed_raw_equivalent_tokens()` to compute raw tokens through the
same code path as the kernel. Token count parity is verified by test.

## Observer Port

```
QualityMetricsObserver (domain port)
├── OTelQualityObserver      → OTLP histograms → Prometheus → Grafana
├── TracingQualityObserver   → structured JSON logs → Loki → Grafana
├── CompositeQualityObserver → fan-out to N backends
└── NoopQualityObserver      → tests / disabled
```

The composition root creates `CompositeQualityObserver(OTel + Tracing)` by default.
Both backends are always active.

## OTel Metrics

### Emitted by `GetContext`

| Metric | Type | Description |
|:-------|:-----|:------------|
| `rehydration.rpc.duration` | f64 histogram (s) | RPC latency |
| `rehydration.bundle.nodes` | u64 histogram | Nodes in bundle |
| `rehydration.bundle.relationships` | u64 histogram | Relationships in bundle |
| `rehydration.bundle.details` | u64 histogram | Node details in bundle |
| `rehydration.rendered.tokens` | u64 histogram | Rendered token count |
| `rehydration.truncation.total` | u64 counter | Renders requiring truncation |
| `rehydration.mode.selected` | u64 counter | Resolved RehydrationMode (label: `mode`) |
| `rehydration.quality.*` | via observer | 5 quality metrics (see below) |
| `rehydration.session.*` | f64/u64 histograms | Timing breakdown (when available) |

### Emitted by `GetContextPath`

| Metric | Type | Description |
|:-------|:-----|:------------|
| `rehydration.rpc.duration` | f64 histogram (s) | RPC latency |
| `rehydration.bundle.nodes` | u64 histogram | Nodes in bundle |
| `rehydration.bundle.relationships` | u64 histogram | Relationships in bundle |
| `rehydration.bundle.details` | u64 histogram | Node details in bundle |
| `rehydration.rendered.tokens` | u64 histogram | Rendered token count |
| `rehydration.truncation.total` | u64 counter | Renders requiring truncation |
| `rehydration.mode.selected` | u64 counter | Resolved RehydrationMode (label: `mode`) |
| `rehydration.quality.*` | via observer | 5 quality metrics (see below) |
| `rehydration.session.*` | f64/u64 histograms | Timing breakdown (when available) |

### Emitted by `RehydrateSession`

| Metric | Type | Description |
|:-------|:-----|:------------|
| `rehydration.rpc.duration` | f64 histogram (s) | RPC latency |
| `rehydration.session.*` | f64/u64 histograms | Timing breakdown |

> `RehydrateSession` does **not yet** emit quality or bundle metrics.
> Per-role rendering is planned.

### Quality Metrics (via observer, `GetContext` + `GetContextPath`)

| Metric | Type | Label |
|:-------|:-----|:------|
| `rehydration.quality.raw_equivalent_tokens` | u64 histogram | `rpc` |
| `rehydration.quality.compression_ratio` | f64 histogram | `rpc` |
| `rehydration.quality.causal_density` | f64 histogram | `rpc` |
| `rehydration.quality.noise_ratio` | f64 histogram | `rpc` |
| `rehydration.quality.detail_coverage` | f64 histogram | `rpc` |

### Timing Breakdown (`GetContext` + `GetContextPath` + `RehydrateSession`)

| Metric | Type | Unit |
|:-------|:-----|:-----|
| `rehydration.session.graph_load.duration` | f64 histogram | seconds |
| `rehydration.session.detail_load.duration` | f64 histogram | seconds |
| `rehydration.session.bundle_assembly.duration` | f64 histogram | seconds |
| `rehydration.session.batch_size` | u64 histogram | — |
| `rehydration.session.role_count` | u64 histogram | — |

### Defined but Not Yet Emitted

These instruments exist in `KernelMetrics` but are not recorded by any RPC handler:

| Metric | Status |
|:-------|:-------|
| `rehydration.projection.lag` | Defined, never recorded (projection runtime does not emit) |

## Structured Logs (Loki)

When `REHYDRATION_LOG_FORMAT=json`, the `TracingQualityObserver` emits:

```json
{
  "timestamp": "2026-03-27T16:47:37Z",
  "level": "INFO",
  "target": "rehydration.quality",
  "fields": {
    "rpc": "GetContext",
    "root_node_id": "node:case:123",
    "role": "developer",
    "quality_raw_equivalent_tokens": 342,
    "quality_compression_ratio": 1.87,
    "quality_causal_density": 0.67,
    "quality_noise_ratio": 0.0,
    "quality_detail_coverage": 0.75
  },
  "message": "bundle quality metrics"
}
```

### LogQL Examples

```logql
# All quality metrics events
{app_kubernetes_io_name="rehydration-kernel"} | json | quality_compression_ratio > 0

# High compression renders
{app_kubernetes_io_name="rehydration-kernel"} | json | quality_compression_ratio > 2.0

# Low causal density (structural-heavy graphs)
{app_kubernetes_io_name="rehydration-kernel"} | json | quality_causal_density < 0.3

# Quality by RPC
{app_kubernetes_io_name="rehydration-kernel"} | json | rpc = "GetContext"
```

### PromQL Examples

The OTel Collector Prometheus exporter is configured with `namespace: rehydration`,
so all metric names are prefixed with `rehydration_`.

```promql
# Average compression ratio
histogram_quantile(0.5, rate(rehydration_quality_compression_ratio_bucket[5m]))

# P99 raw token count
histogram_quantile(0.99, rate(rehydration_quality_raw_equivalent_tokens_bucket[5m]))

# Causal density by RPC
histogram_quantile(0.5, rate(rehydration_quality_causal_density_bucket{rpc="GetContext"}[5m]))

# RPC latency P95
histogram_quantile(0.95, rate(rehydration_rpc_duration_bucket[5m]))
```

## Helm Configuration

Enable the full observability stack in your values overlay:

```yaml
loki:
  enabled: true

grafana:
  enabled: true
  adminPassword: admin  # change in production

otelCollector:
  enabled: true
```

When `otelCollector.enabled=true`, the kernel deployment automatically sets
`OTEL_EXPORTER_OTLP_ENDPOINT` to the in-chart collector service. No manual
configuration needed.

Grafana auto-provisions two datasources:
- **Loki** at `http://<release>-loki:3100`
- **Prometheus** at `http://<release>-otel-collector:9090` (when OTel enabled)

### Accessing Grafana

```bash
kubectl port-forward svc/<release>-grafana 3000:3000 -n <namespace>
# Open http://localhost:3000 (admin/admin by default)
```

> **Security**: Default Grafana deployment enables anonymous admin access for
> development convenience. Disable for production — see [security-model.md](security-model.md).

## Environment Variables

| Variable | Default | Description |
|:---------|:--------|:------------|
| `REHYDRATION_LOG_FORMAT` | `compact` | Log format: `json` (for Loki), `pretty`, `compact` |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | — | OTLP gRPC endpoint (auto-set when otelCollector enabled) |
| `OTEL_EXPORTER_OTLP_CA_PATH` | — | CA certificate for OTLP server verification |
| `OTEL_EXPORTER_OTLP_CERT_PATH` | — | Client certificate for OTLP mTLS |
| `OTEL_EXPORTER_OTLP_KEY_PATH` | — | Client key for OTLP mTLS |
| `OTEL_SERVICE_NAME` | — | Override service name in OTel metadata |
| `RUST_LOG` | `info` | Log level filter |

## Limitations

- `RehydrateSession` does **not yet** emit quality or bundle metrics. Per-role
  rendering with quality metrics is planned
  (see [ROADMAP_MASTER.md](research/ROADMAP_MASTER.md)).
- Quality metrics fan-out is synchronous. Async fan-out is planned to avoid
  latency impact on the gRPC hot path.
- OTLP export supports mTLS via `OTEL_EXPORTER_OTLP_{CA,CERT,KEY}_PATH` env vars. Plaintext by default when no env vars set.
- `rehydration.bundle.details` and `rehydration.projection.lag` are defined
  but not yet recorded by any handler.
- `GetContext` and `GetContextPath` have full OTel parity. `RehydrateSession`
  only emits `rpc.duration` and timing.
