# Documentation Index

Navigation hub for kernel documentation.

Current integration stance:

- `GraphBatch -> translator -> async projection events` is the recommended write path for model-driven producers.
- The stable kernel boundary remains gRPC reads plus async projection subjects.
- The dedicated `repair-judge` is an experimental stabilization helper for model extraction. It is not part of the stable kernel contract.

## Guides

| Document | Content |
|:---------|:--------|
| [usage-guide.md](usage-guide.md) | Getting started: 3 steps to graph-aware LLM context |
| [graph-batch-quickstart.md](graph-batch-quickstart.md) | Fastest path for model-driven graph ingestion |
| [beta-status.md](beta-status.md) | v1beta1 maturity matrix, path to v1, known limitations |
| [security-model.md](security-model.md) | Transport security, threat model, TLS configuration |
| [testing.md](testing.md) | Unit, integration, benchmark, live vLLM smoke, and experimental repair-judge coverage |
| [observability.md](observability.md) | Quality metrics, OTel, Loki, Grafana stack |

## Operations

| Document | Content |
|:---------|:--------|
| [kubernetes-deploy.md](operations/kubernetes-deploy.md) | Helm deployment, values, TLS, observability stack |
| [container-image.md](operations/container-image.md) | OCI image, environment variables, tags |
| [kubernetes-transport-smoke.md](operations/kubernetes-transport-smoke.md) | In-cluster TLS smoke test |
| [deployment-boundary.md](operations/deployment-boundary.md) | What this repo owns vs. what it does not |

## Architecture Decisions

| ADR | Decision |
|:----|:---------|
| [ADR-007](adr/ADR-007-quality-metrics-observability.md) | BundleQualityMetrics as domain VO + hexagonal observer port |
| [ADR-008](adr/ADR-008-graph-batch-ingestion-boundary.md) | GraphBatch as the experimental ingestion boundary |

Six earlier decisions (command/query split, projection model, TLS, compatibility
removal, multi-resolution tiers) are documented in PRs but not yet written as
formal ADRs. See [adr/README.md](adr/README.md) for the source PRs.

## Integration

| Document | Content |
|:---------|:--------|
| [kernel-node-centric-integration-contract.md](migration/kernel-node-centric-integration-contract.md) | Public gRPC + async contract for consumers |
| [kernel-runtime-integration-reference.md](migration/kernel-runtime-integration-reference.md) | Recommended consumer runtime shape |
| [pir-kernel-real-integration-plan.md](migration/pir-kernel-real-integration-plan.md) | Execution plan and slice order before wiring the real PIR runtime |
| [pir-kernel-live-context-consumption-evidence.md](migration/pir-kernel-live-context-consumption-evidence.md) | Live two-wave PIR evidence: publish, rehydrate, and answer from rendered context |
| [graph-batch-ingestion-api.md](graph-batch-ingestion-api.md) | Experimental ingress API proposal over GraphBatch |

## Research

Papers, roadmaps, benchmarks, and incident reports: [research/](research/README.md)

LaTeX submission package: [paper/](paper/README.md)

## Archived

Historical and superseded documents: [archived/](archived/README.md)
