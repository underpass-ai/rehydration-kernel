# Kernel Memory Protocol API Design

Date: 2026-04-30
Status: Draft contract for the Kernel 1.0 public memory slice

## Product Stance

This is not a CRUD API, not a generic graph database API, and not a thin wrapper
around existing transport conventions.

The kernel should define its own protocol for agent memory:

```text
remember what happened
wake up with the right state
ask memory for an evidence-backed answer
trace why the answer is true
inspect the raw substrate when needed
```

gRPC, NATS, and MCP are bindings. They carry the protocol, but they do not
define the product.

The disruptive bet is:

```text
LLMs do not need a database dump.
Humans do not need a graph debugger.
Both need a memory system that can wake them up, prove claims, and show what is
missing.
```

## Core Rule

Every public operation must return two layers:

1. A human/LLM layer: short, direct, usable immediately.
2. A proof layer: evidence, relations, provenance, conflicts, and missing data.

If the proof layer is weak, the answer must say so.

## Protocol Name

Working name:

```text
Kernel Memory Protocol
```

Short name:

```text
KMP
```

The repo may still use `v1beta1` for protobuf package compatibility, but the
product should talk about Kernel Memory Protocol, not "a gRPC service" or "an
MCP wrapper".

## Memory Moves

KMP exposes five memory moves.

| Move | MCP tool | gRPC method | NATS subject | Purpose |
|:-----|:---------|:------------|:-------------|:--------|
| `remember` | `kernel_remember` | `Remember` | `kernel.memory.remember` | Store new memory capsules with claims, links, evidence, and provenance. |
| `wake` | `kernel_wake` | `Wake` | Optional notification only | Return the compact state needed to continue work. |
| `ask` | `kernel_ask` | `Ask` | Not recommended for async MVP | Answer a question from memory, or say what is missing. |
| `trace` | `kernel_trace` | `Trace` | Not recommended for async MVP | Return the relationship path and evidence trail behind an answer. |
| `inspect` | `kernel_inspect` | `Inspect` | Not recommended for async MVP | Show raw graph/detail state for audits and debugging. |

The old descriptive names remain valid as aliases during migration:

| Alias | Canonical move |
|:------|:---------------|
| `kernel_ingest_context` | `kernel_remember` |
| `kernel_explore_context` | `kernel_ask` |
| `kernel_get_context` | `kernel_wake` or advanced bundle read |
| `kernel_inspect_context` | `kernel_inspect` |

## Why These Moves

`remember` is not "insert rows". It records experience.

`wake` is not "get context". It returns the minimal state needed to resume
agency.

`ask` is not search. It requires an answer policy: evidence-backed answer,
conflict, or unknown.

`trace` is not graph traversal for its own sake. It explains why memory reached
a conclusion.

`inspect` is the escape hatch. It exists so the system stays honest.

## Public Mental Model

The public model is a memory capsule.

A memory capsule is the smallest useful packet a human or LLM can write:

```text
about      -> what this memory is anchored to
scope      -> bounded region where it happened
claims     -> statements worth remembering
links      -> typed relationships between claims or objects
evidence   -> text, event ids, tool outputs, logs, or observations
provenance -> who produced it and when
```

`anchor`, `scope`, `entry`, `relation`, and `detail` remain kernel coordinates.
They are important for storage and traversal, but the public API should let a
caller think in capsules.

## Memory Capsule Shape

Canonical JSON:

```json
{
  "about": "question:830ce83f",
  "capsule": {
    "scope": {
      "id": "conversation:rachel-2026-04-12",
      "dimension": "conversation",
      "title": "Rachel relocation discussion"
    },
    "claims": [
      {
        "id": "claim:rachel-denver",
        "text": "Rachel said she was moving to Denver.",
        "time": "2026-04-12T15:00:00Z",
        "sequence": 1
      },
      {
        "id": "claim:rachel-austin",
        "text": "Rachel later corrected the destination to Austin.",
        "time": "2026-04-12T15:05:00Z",
        "sequence": 2
      }
    ],
    "links": [
      {
        "from": "claim:rachel-austin",
        "to": "claim:rachel-denver",
        "rel": "supersedes",
        "class": "evidential",
        "why": "The later statement corrects the earlier destination.",
        "evidence": "Later she corrected it: the move is to Austin.",
        "confidence": "high"
      }
    ],
    "evidence": [
      {
        "id": "evidence:rachel-turn-2",
        "supports": ["claim:rachel-austin"],
        "text": "Later she corrected it: the move is to Austin.",
        "source": "conversation turn 2"
      }
    ]
  },
  "provenance": {
    "source_kind": "agent",
    "source_agent": "longmemeval-adapter",
    "observed_at": "2026-04-30T10:20:03Z",
    "correlation_id": "corr:830ce83f",
    "causation_id": "eval:item:830ce83f"
  },
  "idempotency_key": "remember:830ce83f:1"
}
```

The kernel can translate this into the existing node/detail/relation projection
model. Producers that already emit `GraphBatch` can keep doing so.

## Relations Are Producer Facts

The relationship and its type are produced before storage.

For LLM producers, the LLM must produce:

- the link;
- the relation type;
- the semantic class;
- the evidence or rationale;
- confidence.

The kernel validates, stores, traverses, and exposes the relation. It should not
silently invent relationship meaning during reads.

Allowed semantic classes mirror the current kernel enum:

```text
structural
causal
motivational
procedural
evidential
constraint
```

Recommended relation names are intentionally plain:

```text
contains
precedes
follows
supports
contradicts
supersedes
caused_by
depends_on
mentions
derived_from
attempted_by
verified_by
```

This is not an ontology. It is a working language for memory.

## Move: `remember`

Purpose:

```text
Store a memory capsule.
```

MCP request:

```json
{
  "about": "incident:checkout-latency-2026-04-30",
  "capsule": {
    "scope": {
      "id": "incident:checkout-latency-2026-04-30:triage",
      "dimension": "incident"
    },
    "claims": [
      {
        "id": "claim:redis-timeouts-rise",
        "text": "Redis timeout rate increased after the checkout rollout.",
        "time": "2026-04-30T09:15:00Z",
        "sequence": 1
      }
    ],
    "links": [
      {
        "from": "claim:redis-timeouts-rise",
        "to": "incident:checkout-latency-2026-04-30",
        "rel": "supports",
        "class": "evidential",
        "why": "The timeout increase is evidence for the checkout latency incident.",
        "evidence": "Redis timeout rate increased after the checkout rollout.",
        "confidence": "medium"
      }
    ],
    "evidence": [
      {
        "id": "evidence:redis-timeout-panel",
        "supports": ["claim:redis-timeouts-rise"],
        "text": "Redis timeout p95 rose during the checkout rollout window.",
        "source": "metrics panel"
      }
    ]
  },
  "idempotency_key": "remember:checkout-latency:triage:1"
}
```

MCP response:

```json
{
  "summary": "Remembered 1 claim, 1 link, and 1 evidence item for incident:checkout-latency-2026-04-30.",
  "memory": {
    "about": "incident:checkout-latency-2026-04-30",
    "capsule_id": "capsule:checkout-latency:triage:1",
    "accepted": {
      "claims": 1,
      "links": 1,
      "evidence": 1
    },
    "read_after_write_ready": false
  },
  "warnings": []
}
```

Rules:

- `idempotency_key` is required for non-dry-run requests.
- Acceptance is not read-model completion.
- The same capsule replay with the same idempotency key returns the same
  acceptance.
- The same idempotency key with a different capsule is rejected.
- `dry_run=true` validates and returns the translated projection summary.

## Move: `wake`

Purpose:

```text
Return the compact state needed to continue.
```

This is the most important move. It is the product wedge.

Most memory systems return documents. The kernel should return a wake packet:

```text
what is the objective?
what is currently true?
what caused this state?
what evidence matters?
what is unresolved?
what should the caller do next?
what must the caller not forget?
```

MCP request:

```json
{
  "about": "project:kernel-memory-protocol",
  "role": "implementer",
  "intent": "continue designing the public API",
  "budget": {
    "tokens": 1600,
    "detail": "compact"
  }
}
```

MCP response:

```json
{
  "summary": "Continue with the Kernel Memory Protocol design. The current direction is memory moves first, transports second.",
  "wake": {
    "objective": "Design a public memory API that is simple for humans and LLMs while still binding to MCP, gRPC, and NATS.",
    "current_state": [
      "Kernel 1.0 already has gRPC reads, async projection subjects, graph traversal, and detail retrieval.",
      "The product direction should avoid CRUD naming and expose memory moves."
    ],
    "causal_spine": [
      {
        "claim": "MCP/gRPC/NATS should be bindings, not the product shape.",
        "because": "The product needs to feel like a memory protocol, not a standards checklist.",
        "evidence_ref": "evidence:api-design-direction"
      }
    ],
    "open_loops": [
      "Decide exact proto message names.",
      "Add JSON schemas after the memory moves settle."
    ],
    "next_actions": [
      "Update the action plan terminology from context CRUD to memory moves.",
      "Prototype local MCP read tools over existing gRPC query services."
    ],
    "guardrails": [
      "Do not claim benchmark success before implementation and measurement.",
      "Do not let the kernel invent relation meaning silently during reads."
    ]
  },
  "proof": {
    "evidence": [],
    "missing": [],
    "confidence": "medium"
  },
  "warnings": []
}
```

Wake response fields:

| Field | Meaning |
|:------|:--------|
| `summary` | One paragraph suitable for a human or LLM. |
| `wake.objective` | What the caller is trying to continue. |
| `wake.current_state` | Current facts selected from memory. |
| `wake.causal_spine` | Ordered, evidence-backed reasons that explain the state. |
| `wake.open_loops` | Known unresolved work or uncertainty. |
| `wake.next_actions` | Concrete continuation options. |
| `wake.guardrails` | Constraints that prevent harmful or wrong continuation. |
| `proof` | Evidence, missing data, confidence, and optional raw refs. |

This is different from `GetContext`: `GetContext` returns a bundle;
`wake` returns a continuation state.

## Move: `ask`

Purpose:

```text
Answer from memory, with proof, or refuse certainty.
```

MCP request:

```json
{
  "about": "question:830ce83f",
  "question": "Where did Rachel move after her recent relocation?",
  "answer_policy": "evidence_or_unknown",
  "prefer": {
    "time": "latest",
    "relations": ["supersedes", "supports"]
  }
}
```

MCP response:

```json
{
  "summary": "Rachel moved to Austin. The Austin claim supersedes the earlier Denver claim.",
  "answer": "Austin",
  "because": [
    {
      "claim": "Rachel later corrected the destination to Austin.",
      "evidence": "Later she corrected it: the move is to Austin.",
      "ref": "claim:rachel-austin"
    }
  ],
  "proof": {
    "path": [
      {
        "from": "claim:rachel-austin",
        "to": "claim:rachel-denver",
        "rel": "supersedes",
        "class": "evidential",
        "confidence": "high"
      }
    ],
    "conflicts": [],
    "missing": [],
    "confidence": "high"
  },
  "warnings": []
}
```

Answer policy values:

| Policy | Behavior |
|:-------|:---------|
| `evidence_or_unknown` | Return an answer only when evidence exists. |
| `show_conflicts` | Return competing claims if conflict is unresolved. |
| `best_effort` | Return the best supported answer and mark uncertainty. |

Default policy should be `evidence_or_unknown`.

## Move: `trace`

Purpose:

```text
Explain how memory connects two facts or how an answer was reached.
```

MCP request:

```json
{
  "from": "claim:rachel-austin",
  "to": "claim:rachel-denver",
  "goal": "explain_update",
  "include": {
    "evidence": true,
    "raw_refs": true
  }
}
```

MCP response:

```json
{
  "summary": "claim:rachel-austin supersedes claim:rachel-denver.",
  "trace": [
    {
      "from": "claim:rachel-austin",
      "to": "claim:rachel-denver",
      "rel": "supersedes",
      "class": "evidential",
      "why": "The later statement corrects the earlier destination.",
      "evidence": "Later she corrected it: the move is to Austin.",
      "confidence": "high"
    }
  ],
  "warnings": []
}
```

`trace` should be deterministic. If multiple plausible paths exist, return them
ranked and explain why the top path was selected.

## Move: `inspect`

Purpose:

```text
Show what the kernel actually stored.
```

MCP request:

```json
{
  "ref": "claim:rachel-austin",
  "include": {
    "incoming": true,
    "outgoing": true,
    "details": true,
    "raw": true
  }
}
```

MCP response:

```json
{
  "summary": "Found claim:rachel-austin with 1 outgoing link and 1 evidence item.",
  "object": {
    "ref": "claim:rachel-austin",
    "kind": "claim",
    "text": "Rachel later corrected the destination to Austin."
  },
  "links": {
    "outgoing": [
      {
        "to": "claim:rachel-denver",
        "rel": "supersedes",
        "class": "evidential"
      }
    ],
    "incoming": []
  },
  "evidence": [
    {
      "id": "evidence:rachel-turn-2",
      "text": "Later she corrected it: the move is to Austin."
    }
  ],
  "warnings": []
}
```

## Binding: MCP

MCP is the first-class interactive binding.

Required tools:

```text
kernel_remember
kernel_wake
kernel_ask
kernel_trace
kernel_inspect
```

MCP tool descriptions should be written for LLM tool choice, not for generated
SDKs. They should answer:

- when should the model call this?
- what minimal input is required?
- what proof will come back?
- when should the model call a different memory move?

MCP must not duplicate kernel logic. The MCP server calls the same application
services used by gRPC and NATS ingress.

## Binding: gRPC

gRPC carries the same protocol for typed clients.

Recommended service:

```proto
service KernelMemoryService {
  rpc Remember(RememberRequest) returns (RememberResponse);
  rpc Wake(WakeRequest) returns (WakeResponse);
  rpc Ask(AskRequest) returns (AskResponse);
  rpc Trace(TraceRequest) returns (TraceResponse);
  rpc Inspect(InspectRequest) returns (InspectResponse);
}
```

This service should live additively beside current services:

- `ContextQueryService`
- `ContextCommandService`

Existing RPCs remain valid:

| Existing RPC | KMP relationship |
|:-------------|:-----------------|
| `GetContext` | Low-level bundle read used by `wake` or advanced callers. |
| `GetContextPath` | Low-level path read used by `trace`. |
| `GetNodeDetail` | Low-level detail read used by `inspect`. |
| `RehydrateSession` | Compatibility rehydration call; public product vocabulary should use `wake` and `scope`. |
| `UpdateContext` | Generic command/event-store path; not the preferred memory capsule path. |

The proto should keep the transport typed, but not force the product back into
CRUD names.

## Binding: NATS

NATS carries asynchronous memory ingestion and notifications.

Recommended subjects:

| Subject | Direction | Purpose |
|:--------|:----------|:--------|
| `kernel.memory.remember` | kernel subscribes | Submit a memory capsule asynchronously. |
| `kernel.memory.remembered` | kernel publishes | Capsule accepted after validation and idempotency. |
| `kernel.memory.rejected` | kernel publishes | Capsule rejected with typed reasons. |
| `kernel.memory.wake.generated` | kernel publishes | Optional notification that a wake packet was generated. |
| `graph.node.materialized` | kernel subscribes | Existing low-level projection input. |
| `graph.relation.materialized` | kernel subscribes | Existing relation-only projection input. |
| `node.detail.materialized` | kernel subscribes | Existing detail projection input. |

`kernel.memory.remember` is the simple async path. Existing `graph.*` and
`node.detail.*` subjects remain the lower-level projection contract.

NATS envelope:

```json
{
  "event_id": "evt:memory:remember:830ce83f:1",
  "correlation_id": "corr:830ce83f",
  "causation_id": "eval:item:830ce83f",
  "occurred_at": "2026-04-30T10:20:03Z",
  "schema_version": "kmp.v0",
  "idempotency_key": "remember:830ce83f:1",
  "move": "remember",
  "data": {
    "about": "question:830ce83f",
    "capsule": {},
    "provenance": {}
  }
}
```

NATS acceptance:

```json
{
  "event_id": "evt:memory:remembered:830ce83f:1",
  "correlation_id": "corr:830ce83f",
  "causation_id": "evt:memory:remember:830ce83f:1",
  "occurred_at": "2026-04-30T10:20:04Z",
  "schema_version": "kmp.v0",
  "move": "remembered",
  "data": {
    "about": "question:830ce83f",
    "capsule_id": "capsule:830ce83f:1",
    "accepted": {
      "claims": 2,
      "links": 1,
      "evidence": 1
    },
    "read_after_write_ready": false,
    "warnings": []
  }
}
```

Rules:

- `remembered` means accepted, not immediately readable.
- Producers must send `idempotency_key`.
- Rejections include `code`, `message`, `field`, `ref`, and `retryable`.
- Async `ask` and `trace` are not part of the MVP. They can be added later if
  there is a real workload for asynchronous reasoning.

## Internal Coordinates

The kernel still needs durable traversal coordinates:

```text
anchor
scope
entry
dimension
sequence
timestamp
relation
detail
provenance
```

KMP maps capsules onto those coordinates:

| Capsule field | Kernel coordinate |
|:--------------|:------------------|
| `about` | `anchor_id` / root node id |
| `scope.id` | `scope_id` |
| `scope.dimension` | `dimension` |
| `claims[]` | entries and/or graph nodes |
| `links[]` | graph relationships |
| `evidence[]` | node details and evidence records |
| `provenance` | provenance on nodes, relations, details, and events |

The public API should not force every caller to understand the internal graph
shape. Inspection can expose it when needed.

## Read Semantics

Default read behavior:

- evidence beats summary;
- causal/evidential paths beat nearest-neighbor similarity;
- current/superseding facts beat stale facts when the path proves it;
- conflicts are returned explicitly;
- unknown is a valid answer;
- raw state is hidden unless requested.

For `ask`, the default answer policy is:

```text
answer with proof, or say unknown
```

For `wake`, the default output priority is:

```text
objective
current state
causal spine
open loops
next actions
guardrails
proof
```

## Write Semantics

Writes are idempotent and validation-first.

Required behavior:

- require `idempotency_key`;
- validate references before accepting;
- reject duplicate links inside one capsule;
- reject unresolved link refs unless pending refs are explicitly enabled;
- preserve provenance;
- acknowledge acceptance separately from read-model completion;
- expose dry-run validation.

Dry-run behavior:

- validates the capsule;
- returns projected claims/links/evidence counts;
- does not publish events;
- does not update the read model.

## Error Model

MCP should return normal MCP tool errors for transport/tool failures and domain
warnings for successful-but-uncertain memory results.

gRPC mapping:

| Condition | gRPC status |
|:----------|:------------|
| malformed request | `INVALID_ARGUMENT` |
| missing idempotency key | `INVALID_ARGUMENT` |
| unresolved memory ref | `NOT_FOUND` |
| idempotent replay with same payload | `OK` |
| idempotency key reused with different payload | `ABORTED` |
| validation conflict | `ABORTED` |
| transport auth failure | `UNAUTHENTICATED` or `PERMISSION_DENIED` |
| backend unavailable | `UNAVAILABLE` |
| deadline exceeded | `DEADLINE_EXCEEDED` |

The product-level error language should remain simple:

```text
unknown
conflict
missing_evidence
missing_relation
not_ready
rejected
```

## Security Boundary

KMP is not an authorization system.

For this slice:

- local stdio MCP is acceptable for trusted developer workflows;
- remote MCP must use real authentication and audit logging;
- gRPC keeps the current transport security model;
- NATS producers must be authenticated at the bus boundary;
- `scope` remains a traversal coordinate, not an access-control claim.

`ValidateScope` remains a utility, not an authorization backend.

## What Makes This Different

The differentiated product is not that the kernel has MCP, gRPC, or NATS.

The differentiated product is:

- memory moves instead of CRUD endpoints;
- wake packets instead of context dumps;
- proof as a first-class response field;
- unknown/conflict as honest first-class outcomes;
- LLM-produced relations stored as inspectable facts;
- deterministic traversal over causal/evidential links;
- one protocol that works locally through MCP, synchronously through gRPC, and
  asynchronously through NATS.

## Implementation Order

Recommended order:

1. Rename the public API direction to Kernel Memory Protocol in product docs.
2. Add JSON schemas for `kernel_remember`, `kernel_wake`, `kernel_ask`,
   `kernel_trace`, and `kernel_inspect`.
3. Add examples for conversation memory, incident memory, workflow memory, and
   benchmark memory.
4. Implement local stdio MCP read moves first: `wake`, `ask`, `trace`,
   `inspect`.
5. Map read moves onto existing `GetContext`, `GetContextPath`, and
   `GetNodeDetail`.
6. Add capsule-to-projection translation for `remember`.
7. Add typed gRPC `KernelMemoryService`.
8. Add NATS `kernel.memory.remember/remembered/rejected`.
9. Re-run the benchmark and publish what improves and what still fails.

This order avoids over-promising write-side memory before the wake/ask/trace
experience is real.

## Acceptance Criteria

The design is ready to implement when:

- an LLM can choose the right memory move from tool descriptions;
- a human can write a valid memory capsule by hand;
- `wake` returns a usable continuation state, not just graph data;
- `ask` returns proof or unknown;
- `trace` shows the relation path behind an answer;
- `inspect` can audit the raw stored facts;
- the same memory move can be carried through MCP, gRPC, or NATS;
- existing Kernel 1.0 clients remain valid;
- the docs do not claim benchmark success before implementation and
  measurement.

## Non-Goals

KMP is not:

- a vector database API;
- a generic graph database API;
- a LongMemEval-specific API;
- a PIR-specific API;
- an authorization layer;
- a claim that the kernel can infer missing relations without an LLM producer or
  explicit enrichment step.

The public promise is:

```text
If memory is recorded as claims, links, evidence, time, and provenance, the
kernel can wake a caller up, answer with proof, trace why, and show what is
missing.
```
