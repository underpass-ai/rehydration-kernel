# Documentation Index

Navigation hub for kernel documentation.

## Guides

| Document | Content |
|:---------|:--------|
| [usage-guide.md](usage-guide.md) | Getting started: 3 steps to graph-aware LLM context |
| [beta-status.md](beta-status.md) | v1beta1 maturity matrix, path to v1, known limitations |
| [security-model.md](security-model.md) | Transport security, threat model, TLS configuration |
| [testing.md](testing.md) | 270+ unit tests, 9 integration tests, 4 LLM benchmarks |
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

Six earlier decisions (command/query split, projection model, TLS, compatibility
removal, multi-resolution tiers) are documented in PRs but not yet written as
formal ADRs. See [adr/README.md](adr/README.md) for the source PRs.

## Integration

| Document | Content |
|:---------|:--------|
| [kernel-node-centric-integration-contract.md](migration/kernel-node-centric-integration-contract.md) | Public gRPC + async contract for consumers |
| [kernel-runtime-integration-reference.md](migration/kernel-runtime-integration-reference.md) | Recommended consumer runtime shape |

## Research

Papers, roadmaps, benchmarks, and incident reports: [research/](research/README.md)

LaTeX submission package: [paper/](paper/README.md)

## Archived

Legacy compatibility documents (pre-v1beta1): [archived/](archived/README.md)
