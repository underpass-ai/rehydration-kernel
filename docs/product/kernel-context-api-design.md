# Kernel Memory Protocol API Design

Date: 2026-04-30
Status: Draft contract for the Kernel 1.0 public memory slice

## Product Stance

This is not a CRUD API, not a generic graph database API, and not a thin wrapper
around existing transport conventions.

The kernel should define its own protocol for agent memory:

```text
ingest memory with dimensions, entries, relations, evidence, time, and provenance
wake up with the right state
ask memory for an evidence-backed answer
goto a timestamp, sequence, or memory ref across one or more dimensions
find memory near a timestamp, sequence, or memory ref
rewind memory across one, several, or all dimensions
forward memory across one, several, or all dimensions
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

KMP exposes nine memory moves.

| Move | MCP tool | gRPC method | NATS subject | Purpose |
|:-----|:---------|:------------|:-------------|:--------|
| `ingest` | `kernel_ingest` | `Ingest` | `kernel.memory.ingest` | Submit memory with dimensions, entries, relations, evidence, and provenance. |
| `wake` | `kernel_wake` | `Wake` | Optional notification only | Return the compact state needed to continue work. |
| `ask` | `kernel_ask` | `Ask` | Not recommended for async MVP | Answer a question from memory, or say what is missing. |
| `goto` | `kernel_goto` | `Goto` | Not recommended for async MVP | Jump to memory state at a timestamp, sequence, or memory ref over selected dimensions. |
| `near` | `kernel_near` | `Near` | Not recommended for async MVP | Return the temporal neighborhood around a timestamp, sequence, or memory ref. |
| `rewind` | `kernel_rewind` | `Rewind` | Not recommended for async MVP | Move backward in time over one, several, or all dimensions of a memory. |
| `forward` | `kernel_forward` | `Forward` | Not recommended for async MVP | Move forward in time over one, several, or all dimensions of a memory. |
| `trace` | `kernel_trace` | `Trace` | Not recommended for async MVP | Return the relationship path and evidence trail behind an answer. |
| `inspect` | `kernel_inspect` | `Inspect` | Not recommended for async MVP | Show raw graph/detail state for audits and debugging. |

The old descriptive names remain valid as aliases during migration:

| Alias | Canonical move |
|:------|:---------------|
| `kernel_remember` | `kernel_ingest` |
| `kernel_ingest_context` | `kernel_ingest` |
| `kernel_explore_context` | `kernel_ask` |
| `kernel_get_context` | `kernel_wake` or advanced bundle read |
| `kernel_inspect_context` | `kernel_inspect` |

## Why These Moves

`ingest` is not "insert rows". It accepts validated memory and makes it
traversable.

`wake` is not "get context". It returns the minimal state needed to resume
agency.

`ask` is not search. It requires an answer policy: evidence-backed answer,
conflict, or unknown.

`goto`, `near`, `rewind`, and `forward` are not time filters. They move through
memory over a dimension selection: one dimension, a set of dimensions, or every
dimension of the memory.

`trace` is not graph traversal for its own sake. It explains why memory reached
a conclusion.

`inspect` is the escape hatch. It exists so the system stays honest.

## Public Mental Model

The public write model is `memory`.

Memory is the smallest useful packet a human or LLM can ingest:

```text
about      -> what this memory is anchored to
dimensions -> application-neutral planes where entries can live
entries    -> claims, observations, actions, tool calls, logs, or states
relations  -> typed relationships between entries or objects
evidence   -> text, event ids, tool outputs, logs, or observations
provenance -> who produced it and when
```

`anchor`, `dimension`, `scope`, `entry`, `time`, `relation`, and `detail`
remain kernel coordinates. They are important for storage and traversal, but
the public API should let a caller think in memory.

## Dimensions And Time

Dimensions say where memory lives. Time says how memory changes across those
dimensions.

Time is not just another dimension. It is a transversal axis over all dimensions
of a memory. The same entry may have different temporal coordinates in different
dimensions:

```text
conversation sequence
entity valid_from / valid_until
workflow step order
incident occurred_at
kernel ingested_at
producer observed_at
```

KMP must therefore support temporal traversal at three levels:

```text
one dimension
a selected set of dimensions
all dimensions of a memory
```

Temporal movement has three forms:

```text
goto   -> state at cursor
near   -> neighborhood around cursor
rewind -> state before cursor
forward -> state after cursor
```

Supported temporal coordinates:

| Coordinate | Meaning |
|:-----------|:--------|
| `occurred_at` | When the source event happened. |
| `observed_at` | When the producer observed it. |
| `ingested_at` | When the kernel accepted it. |
| `valid_from` | When the entry became true or current. |
| `valid_until` | When the entry stopped being true or current. |
| `sequence` | Order within one scope or dimension. |

## Memory Shape

Canonical JSON:

```json
{
  "about": "question:830ce83f",
  "memory": {
    "dimensions": [
      {
        "id": "conversation:rachel-2026-04-12",
        "kind": "conversation",
        "title": "Rachel relocation discussion"
      },
      {
        "id": "person:rachel",
        "kind": "entity",
        "title": "Rachel"
      },
      {
        "id": "longmemeval:item:830ce83f",
        "kind": "benchmark_record"
      }
    ],
    "entries": [
      {
        "id": "claim:rachel-denver",
        "kind": "claim",
        "text": "Rachel said she was moving to Denver.",
        "coordinates": [
          {
            "dimension": "conversation",
            "scope_id": "conversation:rachel-2026-04-12",
            "sequence": 1,
            "occurred_at": "2026-04-12T15:00:00Z"
          },
          {
            "dimension": "entity",
            "scope_id": "person:rachel",
            "valid_from": "2026-04-12T15:00:00Z"
          }
        ]
      },
      {
        "id": "claim:rachel-austin",
        "kind": "claim",
        "text": "Rachel later corrected the destination to Austin.",
        "coordinates": [
          {
            "dimension": "conversation",
            "scope_id": "conversation:rachel-2026-04-12",
            "sequence": 2,
            "occurred_at": "2026-04-12T15:05:00Z"
          },
          {
            "dimension": "entity",
            "scope_id": "person:rachel",
            "valid_from": "2026-04-12T15:05:00Z"
          },
          {
            "dimension": "benchmark_record",
            "scope_id": "longmemeval:item:830ce83f",
            "sequence": 7
          }
        ]
      }
    ],
    "relations": [
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
  "idempotency_key": "ingest:830ce83f:1"
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

## Move: `ingest`

Purpose:

```text
Submit memory for validation and traversal.
```

MCP request:

```json
{
  "about": "incident:checkout-latency-2026-04-30",
  "memory": {
    "dimensions": [
      {
        "id": "incident:checkout-latency-2026-04-30:triage",
        "kind": "incident"
      },
      {
        "id": "deployment:checkout-rollout",
        "kind": "deployment"
      }
    ],
    "entries": [
      {
        "id": "claim:redis-timeouts-rise",
        "kind": "claim",
        "text": "Redis timeout rate increased after the checkout rollout.",
        "coordinates": [
          {
            "dimension": "incident",
            "scope_id": "incident:checkout-latency-2026-04-30:triage",
            "sequence": 1,
            "occurred_at": "2026-04-30T09:15:00Z"
          },
          {
            "dimension": "deployment",
            "scope_id": "deployment:checkout-rollout",
            "occurred_at": "2026-04-30T09:15:00Z"
          }
        ]
      }
    ],
    "relations": [
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
  "idempotency_key": "ingest:checkout-latency:triage:1"
}
```

MCP response:

```json
{
  "summary": "Ingested 1 entry, 1 relation, and 1 evidence item for incident:checkout-latency-2026-04-30.",
  "memory": {
    "about": "incident:checkout-latency-2026-04-30",
    "memory_id": "memory:checkout-latency:triage:1",
    "accepted": {
      "entries": 1,
      "relations": 1,
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
- The same memory replay with the same idempotency key returns the same
  acceptance.
- The same idempotency key with different memory is rejected.
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

## Move: `goto`

Purpose:

```text
Jump to the memory state at one concrete moment, across one, several, or all
dimensions.
```

MCP request:

```json
{
  "about": "question:830ce83f",
  "at": {
    "time": "2026-04-12T15:03:00Z"
  },
  "dimensions": {
    "mode": "all"
  },
  "include": {
    "evidence": true,
    "relations": true
  }
}
```

MCP response:

```json
{
  "summary": "At 2026-04-12T15:03:00Z, memory still supported Denver as Rachel's destination.",
  "temporal": {
    "direction": "goto",
    "at": {
      "time": "2026-04-12T15:03:00Z"
    }
  },
  "coverage": {
    "requested": {
      "mode": "all"
    },
    "included": ["conversation", "entity", "benchmark_record"],
    "missing": []
  },
  "entries": [
    {
      "ref": "claim:rachel-denver",
      "text": "Rachel said she was moving to Denver.",
      "coordinates": [
        {
          "dimension": "conversation",
          "scope_id": "conversation:rachel-2026-04-12",
          "sequence": 1,
          "occurred_at": "2026-04-12T15:00:00Z"
        },
        {
          "dimension": "entity",
          "scope_id": "person:rachel",
          "valid_from": "2026-04-12T15:00:00Z"
        }
      ]
    }
  ],
  "proof": {
    "path": [],
    "evidence": [],
    "conflicts": [],
    "missing": [],
    "confidence": "high"
  },
  "warnings": []
}
```

## Move: `near`

Purpose:

```text
Return the temporal neighborhood around one concrete moment, sequence, or ref,
across one, several, or all dimensions.
```

Use `goto` when the caller needs the state at a point. Use `near` when the
caller needs what happened around that point.

MCP request:

```json
{
  "about": "question:830ce83f",
  "around": {
    "time": "2026-04-12T15:03:00Z"
  },
  "window": {
    "before_entries": 2,
    "after_entries": 2
  },
  "dimensions": {
    "mode": "only",
    "include": ["conversation", "entity"]
  },
  "include": {
    "evidence": true,
    "relations": true
  }
}
```

MCP response:

```json
{
  "summary": "Near 2026-04-12T15:03:00Z, Denver appeared before the cursor and Austin appeared after it.",
  "temporal": {
    "direction": "near",
    "around": {
      "time": "2026-04-12T15:03:00Z"
    },
    "window": {
      "before_entries": 2,
      "after_entries": 2
    }
  },
  "coverage": {
    "requested": {
      "mode": "only",
      "include": ["conversation", "entity"]
    },
    "included": ["conversation", "entity"],
    "missing": []
  },
  "entries": [
    {
      "ref": "claim:rachel-denver",
      "text": "Rachel said she was moving to Denver.",
      "position": "before",
      "coordinates": [
        {
          "dimension": "conversation",
          "scope_id": "conversation:rachel-2026-04-12",
          "sequence": 1,
          "occurred_at": "2026-04-12T15:00:00Z"
        }
      ]
    },
    {
      "ref": "claim:rachel-austin",
      "text": "Rachel later corrected the destination to Austin.",
      "position": "after",
      "coordinates": [
        {
          "dimension": "conversation",
          "scope_id": "conversation:rachel-2026-04-12",
          "sequence": 2,
          "occurred_at": "2026-04-12T15:05:00Z"
        }
      ]
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
    "evidence": [],
    "conflicts": [],
    "missing": [],
    "confidence": "high"
  },
  "warnings": []
}
```

## Move: `rewind`

Purpose:

```text
Move backward through memory before a cursor, across one, several, or all
dimensions.
```

MCP request:

```json
{
  "about": "question:830ce83f",
  "from": {
    "ref": "claim:rachel-austin"
  },
  "dimensions": {
    "mode": "only",
    "include": ["conversation", "entity"]
  },
  "limit": {
    "entries": 5
  },
  "include": {
    "evidence": true,
    "relations": true
  }
}
```

MCP response:

```json
{
  "summary": "Before claim:rachel-austin, the active supported location was Denver across conversation and entity dimensions.",
  "temporal": {
    "direction": "rewind",
    "from": {
      "ref": "claim:rachel-austin",
      "time": "2026-04-12T15:05:00Z"
    }
  },
  "coverage": {
    "requested": {
      "mode": "only",
      "include": ["conversation", "entity"]
    },
    "included": ["conversation", "entity"],
    "missing": []
  },
  "entries": [
    {
      "ref": "claim:rachel-denver",
      "text": "Rachel said she was moving to Denver.",
      "coordinates": [
        {
          "dimension": "conversation",
          "scope_id": "conversation:rachel-2026-04-12",
          "sequence": 1,
          "occurred_at": "2026-04-12T15:00:00Z"
        },
        {
          "dimension": "entity",
          "scope_id": "person:rachel",
          "valid_from": "2026-04-12T15:00:00Z"
        }
      ]
    }
  ],
  "proof": {
    "path": [],
    "evidence": [],
    "conflicts": [],
    "missing": [],
    "confidence": "high"
  },
  "warnings": []
}
```

## Move: `forward`

Purpose:

```text
Move forward through memory after a cursor, across one, several, or all
dimensions.
```

MCP request:

```json
{
  "about": "question:830ce83f",
  "from": {
    "ref": "claim:rachel-denver"
  },
  "dimensions": {
    "mode": "all"
  },
  "limit": {
    "entries": 5
  }
}
```

MCP response:

```json
{
  "summary": "After claim:rachel-denver, claim:rachel-austin superseded it.",
  "temporal": {
    "direction": "forward",
    "from": {
      "ref": "claim:rachel-denver",
      "time": "2026-04-12T15:00:00Z"
    }
  },
  "coverage": {
    "requested": {
      "mode": "all"
    },
    "included": ["conversation", "entity", "benchmark_record"],
    "missing": []
  },
  "entries": [
    {
      "ref": "claim:rachel-austin",
      "text": "Rachel later corrected the destination to Austin.",
      "coordinates": [
        {
          "dimension": "conversation",
          "scope_id": "conversation:rachel-2026-04-12",
          "sequence": 2,
          "occurred_at": "2026-04-12T15:05:00Z"
        },
        {
          "dimension": "entity",
          "scope_id": "person:rachel",
          "valid_from": "2026-04-12T15:05:00Z"
        }
      ]
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
    "evidence": [],
    "conflicts": [],
    "missing": [],
    "confidence": "high"
  },
  "warnings": []
}
```

Dimension selection values:

| Mode | Meaning |
|:-----|:--------|
| `all` | Traverse every dimension attached to the memory. |
| `only` | Traverse only the listed dimensions. |
| `except` | Traverse every dimension except the listed dimensions. |

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
kernel_ingest
kernel_wake
kernel_ask
kernel_goto
kernel_near
kernel_rewind
kernel_forward
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
  rpc Ingest(IngestRequest) returns (IngestResponse);
  rpc Wake(WakeRequest) returns (WakeResponse);
  rpc Ask(AskRequest) returns (AskResponse);
  rpc Goto(TemporalMoveRequest) returns (TemporalMoveResponse);
  rpc Near(TemporalNearRequest) returns (TemporalMoveResponse);
  rpc Rewind(TemporalMoveRequest) returns (TemporalMoveResponse);
  rpc Forward(TemporalMoveRequest) returns (TemporalMoveResponse);
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
| `UpdateContext` | Generic command/event-store path; not the preferred memory ingest path. |

The proto should keep the transport typed, but not force the product back into
CRUD names.

## Binding: NATS

NATS carries asynchronous memory ingestion and notifications.

Recommended subjects:

| Subject | Direction | Purpose |
|:--------|:----------|:--------|
| `kernel.memory.ingest` | kernel subscribes | Submit memory asynchronously. |
| `kernel.memory.ingested` | kernel publishes | Memory accepted after validation and idempotency. |
| `kernel.memory.rejected` | kernel publishes | Memory rejected with typed reasons. |
| `kernel.memory.wake.generated` | kernel publishes | Optional notification that a wake packet was generated. |
| `graph.node.materialized` | kernel subscribes | Existing low-level projection input. |
| `graph.relation.materialized` | kernel subscribes | Existing relation-only projection input. |
| `node.detail.materialized` | kernel subscribes | Existing detail projection input. |

`kernel.memory.ingest` is the simple async path. Existing `graph.*` and
`node.detail.*` subjects remain the lower-level projection contract.

NATS envelope:

```json
{
  "event_id": "evt:memory:ingest:830ce83f:1",
  "correlation_id": "corr:830ce83f",
  "causation_id": "eval:item:830ce83f",
  "occurred_at": "2026-04-30T10:20:03Z",
  "schema_version": "kmp.v0",
  "idempotency_key": "ingest:830ce83f:1",
  "move": "ingest",
  "data": {
    "about": "question:830ce83f",
    "memory": {},
    "provenance": {}
  }
}
```

NATS acceptance:

```json
{
  "event_id": "evt:memory:ingested:830ce83f:1",
  "correlation_id": "corr:830ce83f",
  "causation_id": "evt:memory:ingest:830ce83f:1",
  "occurred_at": "2026-04-30T10:20:04Z",
  "schema_version": "kmp.v0",
  "move": "ingested",
  "data": {
    "about": "question:830ce83f",
    "memory_id": "memory:830ce83f:1",
    "accepted": {
      "entries": 2,
      "relations": 1,
      "evidence": 1
    },
    "read_after_write_ready": false,
    "warnings": []
  }
}
```

Rules:

- `ingested` means accepted, not immediately readable.
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

KMP maps memory onto those coordinates:

| Memory field | Kernel coordinate |
|:--------------|:------------------|
| `about` | `anchor_id` / root node id |
| `memory.dimensions[].id` | `dimension_id` / `scope_id` |
| `memory.dimensions[].kind` | `dimension` |
| `memory.entries[]` | entries and/or graph nodes |
| `memory.entries[].coordinates[]` | scope, dimension, sequence, and time coordinates |
| `memory.relations[]` | graph relationships |
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
- reject duplicate relations inside one memory ingest;
- reject unresolved link refs unless pending refs are explicitly enabled;
- reject entry coordinates that reference dimensions absent from the memory;
- preserve provenance;
- acknowledge acceptance separately from read-model completion;
- expose dry-run validation.

Dry-run behavior:

- validates the memory;
- returns projected entry/relation/evidence counts;
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
- temporal goto/near/rewind/forward across selected memory dimensions;
- proof as a first-class response field;
- unknown/conflict as honest first-class outcomes;
- LLM-produced relations stored as inspectable facts;
- deterministic traversal over causal/evidential links;
- one protocol that works locally through MCP, synchronously through gRPC, and
  asynchronously through NATS.

## Implementation Order

Recommended order:

1. Rename the public API direction to Kernel Memory Protocol in product docs.
2. Add JSON schemas for `kernel_ingest`, `kernel_wake`, `kernel_ask`,
   `kernel_goto`, `kernel_near`, `kernel_rewind`, `kernel_forward`,
   `kernel_trace`, and `kernel_inspect`.
   Draft fixtures live under
   [`api/examples/kernel/v1beta1/kmp`](../../api/examples/kernel/v1beta1/kmp).
3. Add examples for conversation memory, incident memory, workflow memory, and
   benchmark memory.
4. Implement local stdio MCP read moves first: `wake`, `ask`, `trace`,
   `inspect`. The first adapter lives in
   [`crates/rehydration-mcp`](../../crates/rehydration-mcp) and supports
   fixture-backed mode plus live gRPC reads through
   `REHYDRATION_KERNEL_GRPC_ENDPOINT`.
5. Map read moves onto existing `GetContext`, `GetContextPath`, and
   `GetNodeDetail`.
6. Add memory-to-projection translation for `ingest`.
7. Add typed gRPC `KernelMemoryService`.
8. Add NATS `kernel.memory.ingest/ingested/rejected`.
9. Re-run the benchmark and publish what improves and what still fails.

This order avoids over-promising write-side memory before the wake/ask/trace
experience is real.

## Acceptance Criteria

The design is ready to implement when:

- an LLM can choose the right memory move from tool descriptions;
- a human can write valid memory by hand;
- `wake` returns a usable continuation state, not just graph data;
- `ask` returns proof or unknown;
- `goto`, `near`, `rewind`, and `forward` traverse one, several, or all
  dimensions by time;
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
If memory is ingested as dimensions, entries, relations, evidence, time, and
provenance, the kernel can wake a caller up, answer with proof, move through
time, trace why, and show what is missing.
```
