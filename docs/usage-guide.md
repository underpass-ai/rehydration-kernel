# Usage Guide

How to use the Rehydration Kernel to give your AI agent graph-aware context.

## What the Kernel Does

The kernel stores knowledge as a **graph** (nodes + relationships) and serves
**rehydrated context** — a rendered, token-budgeted text bundle that an LLM
can consume directly.

```
Your product            Rehydration Kernel              Infrastructure
┌──────────┐           ┌──────────────────┐           ┌─────────────┐
│           │  gRPC     │                  │           │   Neo4j     │
│  Agent /  │──────────▶│  GetContext       │──────────▶│   (graph)   │
│  LLM app  │◀──────────│  GetContextPath   │◀──────────│             │
│           │  rendered │  RehydrateSession │           ├─────────────┤
│           │  context  │  UpdateContext    │           │   Valkey    │
└──────────┘           └──────────────────┘           │  (details)  │
                                                       ├─────────────┤
                                                       │   NATS      │
                                                       │  (events)   │
                                                       └─────────────┘
```

**You provide**: a graph (nodes, relationships, details) via projection events.
**You get back**: rendered context text, ready for your LLM prompt.

The kernel is **LLM-agnostic**. It does not call any LLM. Your product
decides which LLM to use and how to prompt it.

## Quick Start: 3 Steps

### Step 1 — Seed your graph

Publish projection events to NATS. Each event materializes a node or detail
in the kernel's graph store.

**Materialize a node:**

```json
{
  "event_id": "evt-1",
  "event_type": "graph.node.materialized",
  "payload": {
    "node_id": "task-42",
    "node_kind": "task",
    "title": "Fix payment retry logic",
    "summary": "Implement bounded retries with exponential backoff.",
    "status": "in_progress",
    "labels": ["payments", "resilience"]
  }
}
```

**Materialize a relationship:**

```json
{
  "event_id": "evt-2",
  "event_type": "graph.node.materialized",
  "payload": {
    "node_id": "decision-7",
    "node_kind": "decision",
    "title": "Use exponential backoff",
    "summary": "Chosen over fixed interval to reduce provider load.",
    "status": "accepted",
    "relations": [
      {
        "target_node_id": "task-42",
        "relation_type": "AUTHORIZES",
        "semantic_class": "motivational",
        "rationale": "exponential backoff reduces provider throttling risk"
      }
    ]
  }
}
```

**Materialize node detail (extended context):**

```json
{
  "event_id": "evt-3",
  "event_type": "node.detail.materialized",
  "payload": {
    "node_id": "task-42",
    "detail": "The retry handler wraps settlement API calls with capped exponential backoff (base=1s, max=30s, cap=5 retries). On final failure, it publishes a dead-letter event and alerts the payments oncall channel.",
    "content_hash": "sha256:abc123",
    "revision": 1
  }
}
```

### Step 2 — Query context

Call the kernel via gRPC to get rendered context for your LLM.

**GetContext** — full context around a root node:

```bash
grpcurl -plaintext localhost:50051 \
  underpass.rehydration.kernel.v1beta1.ContextQueryService/GetContext \
  -d '{
    "root_node_id": "task-42",
    "role": "developer",
    "token_budget": 2000,
    "depth": 3
  }'
```

**Response** (simplified):

```json
{
  "bundle": {
    "root_node_id": "task-42",
    "bundles": [{
      "role": "developer",
      "root_node": { "title": "Fix payment retry logic", ... },
      "neighbor_nodes": [{ "title": "Use exponential backoff", ... }],
      "relationships": [{
        "relationship_type": "AUTHORIZES",
        "explanation": {
          "semantic_class": "motivational",
          "rationale": "exponential backoff reduces provider throttling risk"
        }
      }],
      "node_details": [{
        "node_id": "task-42",
        "detail": "The retry handler wraps settlement API calls..."
      }]
    }]
  },
  "rendered": {
    "content": "Root: Fix payment retry logic (in_progress)\n\nDecision: Use exponential backoff\n  → AUTHORIZES task-42 [motivational]\n  Rationale: exponential backoff reduces provider throttling risk\n\nDetail: The retry handler wraps settlement API calls with capped exponential backoff...",
    "token_count": 187,
    "tiers": [
      { "tier": "L0_SUMMARY", "content": "Objective: Fix payment retry logic..." },
      { "tier": "L1_CAUSAL_SPINE", "content": "Decision: Use exponential backoff..." },
      { "tier": "L2_EVIDENCE_PACK", "content": "Detail: The retry handler wraps..." }
    ]
  }
}
```

### Step 3 — Feed your LLM

Take `rendered.content` (or specific tiers) and include it in your LLM prompt:

```python
context = response.rendered.content

prompt = f"""You are reviewing a task. Here is the rehydrated context
from the project's knowledge graph:

{context}

Question: Is the retry strategy appropriate for this payment provider?
Cite the rationale from the context in your answer."""

answer = llm.chat(prompt)
```

That's it. The kernel handles graph traversal, salience ordering, token
budgeting, and multi-resolution rendering. Your code just calls gRPC and
feeds the text to your LLM.

## Sequence Diagrams

### Basic query flow

```
Agent                Kernel (gRPC)          Neo4j         Valkey
  │                      │                    │              │
  │  GetContext(root=X)  │                    │              │
  │─────────────────────▶│                    │              │
  │                      │  load_neighborhood │              │
  │                      │───────────────────▶│              │
  │                      │◀───────────────────│              │
  │                      │                    │              │
  │                      │  load_node_details_batch (MGET)   │
  │                      │──────────────────────────────────▶│
  │                      │◀──────────────────────────────────│
  │                      │                    │              │
  │                      │  render + truncate │              │
  │                      │  (salience order,  │              │
  │                      │   token budget,    │              │
  │                      │   multi-tier)      │              │
  │                      │                    │              │
  │  RenderedContext      │                    │              │
  │◀─────────────────────│                    │              │
  │                      │                    │              │
  │  feed to LLM ──────▶ (your LLM, your prompt)
```

### Projection event flow (seeding the graph)

```
Your service           NATS              Kernel projection      Neo4j / Valkey
  │                     │                 consumer                │
  │  publish event      │                    │                    │
  │────────────────────▶│                    │                    │
  │                     │  deliver           │                    │
  │                     │───────────────────▶│                    │
  │                     │                    │  upsert node       │
  │                     │                    │───────────────────▶│
  │                     │                    │  upsert detail     │
  │                     │                    │───────────────────▶│
  │                     │                    │                    │
  │                     │                    │  ack               │
  │                     │◀───────────────────│                    │
```

### Multi-role rehydration (P1 optimization)

```
Agent                Kernel                   Neo4j         Valkey
  │                      │                      │              │
  │  RehydrateSession    │                      │              │
  │  roles=[dev,review]  │                      │              │
  │─────────────────────▶│                      │              │
  │                      │  load_neighborhood   │              │
  │                      │  (1 read, shared)    │              │
  │                      │─────────────────────▶│              │
  │                      │◀─────────────────────│              │
  │                      │                      │              │
  │                      │  load_details_batch  │              │
  │                      │  (1 MGET, shared)    │              │
  │                      │─────────────────────────────────── ▶│
  │                      │◀────────────────────────────────────│
  │                      │                      │              │
  │                      │  build bundle × 2   │              │
  │                      │  (clone, ~0.3ms)    │              │
  │                      │                      │              │
  │  2 bundles (1/role)  │                      │              │
  │◀─────────────────────│                      │              │
```

## RPCs at a Glance

| RPC | Use when | Key params |
|-----|----------|------------|
| `GetContext` | You need full context around a node | `root_node_id`, `role`, `depth`, `token_budget` |
| `GetContextPath` | You need context along a specific path (A → B) | `root_node_id`, `target_node_id`, `role` |
| `GetNodeDetail` | You need the extended detail for one node | `node_id` |
| `RehydrateSession` | You need bundles for multiple roles at once | `root_node_id`, `roles[]`, `persist_snapshot` |
| `UpdateContext` | You need to record a context change event | `root_node_id`, `role`, `changes[]`, `metadata` |

## Multi-Resolution Tiers

The kernel renders context at three levels of detail:

| Tier | Content | Typical size | Use when |
|------|---------|:------------:|----------|
| **L0 Summary** | Objective, status, blocker, next action | ~100 tokens | Status checks, dashboards |
| **L1 Causal Spine** | Root + focus + explanatory relationships | ~500 tokens | Diagnosis, decision review |
| **L2 Evidence Pack** | Structural relations, neighbors, full details | Remaining budget | Deep analysis, audit |

Request specific tiers with `max_tier`:

```json
{ "root_node_id": "task-42", "role": "ops", "max_tier": "L1_CAUSAL_SPINE", "token_budget": 800 }
```

## Token Budget

The kernel enforces a token budget using the `cl100k_base` BPE tokenizer.
Content is ordered by **salience** (causal > motivational > evidential >
constraint > procedural > structural) and truncated when the budget is
exceeded. The response includes `token_count` so you know exactly how many
tokens the rendered context uses.

## What the Kernel Is NOT

- **Not an LLM** — it does not generate text, only structures and renders context
- **Not a RAG system** — it does not do similarity search; it traverses a typed graph
- **Not a vector database** — relationships have semantic classes and rationale, not embeddings
- **Not tied to any model** — works with GPT, Claude, Llama, Qwen, or any LLM

## Further Reading

- [Proto contracts](../api/proto/underpass/rehydration/kernel/v1beta1/) — the gRPC API definition
- [Reference fixtures](../api/examples/kernel/v1beta1/grpc/) — example request/response JSON
- [Inference prompt examples](../api/examples/inference-prompts/) — how to feed context to your LLM
- [Integration contract](./migration/kernel-node-centric-integration-contract.md) — stability rules
- [Beta status](./beta-status.md) — RPC maturity and known limitations
- [Benchmark](./benchmark-paper-use-cases.md) — LLM-as-judge evaluation methodology
