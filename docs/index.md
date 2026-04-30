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

## Product Plans

| Document | Content |
|:---------|:--------|
| [kernel-context-traversal-and-mcp-action-plan.md](product/kernel-context-traversal-and-mcp-action-plan.md) | Public Kernel 1.0 plan for Kernel Memory Protocol, MCP tools, and honest benchmark follow-up |
| [kernel-context-api-design.md](product/kernel-context-api-design.md) | Kernel Memory Protocol design: remember, wake, ask, trace, inspect, and MCP/gRPC/NATS bindings |

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
| [pir-kernel-graph-inspection-smoke-reranker.md](migration/pir-kernel-graph-inspection-smoke-reranker.md) | Full kernel graph dump for one live PIR incident, with node details, diagram, and analysis |
| [pir-kernel-graph-inspection-smoke-late-waves.md](migration/pir-kernel-graph-inspection-smoke-late-waves.md) | Full kernel graph dump for one live PIR incident after late operational waves and truthful post-stage root projection |
| [pir-first-event-driven-agent-plan.md](migration/pir-first-event-driven-agent-plan.md) | Detailed next-session plan for the first event-driven PIR agent with runtime, local graph reads, bounded iterations, and escalation on unresolved tasks |
| [pir-kernel-sequential-graph-shape-proposal.md](migration/pir-kernel-sequential-graph-shape-proposal.md) | Proposed shift from incident-star graphs to a finding→decision→task→verification semantic spine |
| [pir-kernel-relation-materialized-rfc.md](migration/pir-kernel-relation-materialized-rfc.md) | Proposed additive async subject for relation-only materialization across PIR waves |
| [pir-kernel-blind-structural-evidence.md](migration/pir-kernel-blind-structural-evidence.md) | Live blind-structure evidence: weaker fixture, scorecard before/after reranking |
| [pir-kernel-blind-context-consumption-evidence.md](migration/pir-kernel-blind-context-consumption-evidence.md) | Live blind-consumption evidence: weaker graph, kernel rehydration, and correct downstream answer |
| [graph-batch-ingestion-api.md](graph-batch-ingestion-api.md) | Experimental ingress API proposal over GraphBatch |

## Research

Papers, roadmaps, benchmarks, and incident reports: [research/](research/README.md)

LaTeX submission package: [paper/](paper/README.md)

## Archived

Historical and superseded documents: [archived/](archived/README.md)
