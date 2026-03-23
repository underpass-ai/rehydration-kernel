# Documentation

Kernel documentation lives here.

## Key Documents

- [`PLAN_GRAPH_EXPLORER.md`](./PLAN_GRAPH_EXPLORER.md)
- [`REQUIREMENTS_GRAPH_EXPLORER.md`](./REQUIREMENTS_GRAPH_EXPLORER.md)
- [`BUG_DEPTH_TRAVERSAL.md`](./BUG_DEPTH_TRAVERSAL.md)
- [`PAPER_AGENTIC_CONTEXT_REHYDRATION.md`](./PAPER_AGENTIC_CONTEXT_REHYDRATION.md)
- [`PAPER_SUBMISSION_DRAFT.md`](./PAPER_SUBMISSION_DRAFT.md)
- [`paper/README.md`](./paper/README.md)
- [`paper/acl/README.md`](./paper/acl/README.md)
- [`RELATION_EXPLANATION_MODEL.md`](./RELATION_EXPLANATION_MODEL.md)
- [`ROADMAP_SOTA_CONTEXT_REHYDRATION.md`](./ROADMAP_SOTA_CONTEXT_REHYDRATION.md)

## Paper Status

The current paper track is organized as follows:

- working notes and design rationale:
  [`PAPER_AGENTIC_CONTEXT_REHYDRATION.md`](./PAPER_AGENTIC_CONTEXT_REHYDRATION.md)
- submission-oriented manuscript source:
  [`PAPER_SUBMISSION_DRAFT.md`](./PAPER_SUBMISSION_DRAFT.md)
- ACL review/preprint package:
  [`paper/acl/README.md`](./paper/acl/README.md)
- generated ACL review PDF:
  [`paper/acl/main.pdf`](./paper/acl/main.pdf)
- generated ACL preprint PDF:
  [`paper/acl/main-preprint.pdf`](./paper/acl/main-preprint.pdf)
- experimental artifact outputs:
  [`../artifacts/paper-use-cases/results.md`](../artifacts/paper-use-cases/results.md)
  [`../artifacts/paper-use-cases/summary.json`](../artifacts/paper-use-cases/summary.json)

The current manuscript evaluates four use cases and their ablations:

- failure diagnosis with rehydration-point recovery
- why-implementation reconstruction
- interrupted handoff and resumable execution
- constraint-preserving retrieval under token pressure

## Operations

Operational runbooks and deploy guidance live under
[`docs/operations`](./operations/README.md).

Current explorer-related entries:

- [`operations/graph-explorer-demo.md`](./operations/graph-explorer-demo.md)
- [`operations/kubernetes-deploy.md`](./operations/kubernetes-deploy.md)
- [`operations/kubernetes-transport-smoke.md`](./operations/kubernetes-transport-smoke.md)

## Other Areas

- [`adr`](./adr/README.md)
- [`migration`](./migration/README.md) historical and review-heavy after the
  `v1beta1` cut
- [`runbooks`](./runbooks/README.md)
