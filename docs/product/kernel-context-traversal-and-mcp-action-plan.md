# Kernel Context Traversal And MCP Action Plan

Date: 2026-04-30

## Purpose

This document defines the next capability the kernel must provide: simple,
generic, database-like context ingestion and exploration.

The kernel should let any Underpass project write context once and later explore
it by:

- anchor;
- scope;
- dimension;
- time;
- sequence;
- relation path;
- provenance.

This must work for conversations, user history, production incidents, agent
actions, workflow attempts, benchmark records, and future applications.

The core rule is:

```text
Simple API outside, rich traversal inside.
```

## Public Status

This is a Kernel 1.0 evolution plan, not a separate rewrite.

Kernel 1.0 already has:

- a node-centric graph read path;
- async projection ingestion;
- gRPC query APIs;
- Neo4j-backed relationship traversal;
- Valkey-backed detail retrieval;
- rendered context bundles with semantic relationship metadata.

Kernel 1.0 does not yet have:

- a simple public context database API;
- first-class `anchor/scope/entry` ingestion;
- MCP tools for generic context exploration;
- generic temporal traversal across scopes;
- a public, honest demo that proves before/after, update, and multi-scope
  retrieval without app-specific code.

This plan closes that gap using the existing Kernel 1.0 architecture. It does
not require an embedded database, a new storage stack, or a GPU-bound traversal
pipeline.

## Product Thesis

The kernel remains application-agnostic, but it must become traversal-aware.

Applications should not need to reconstruct context from their own databases
before the kernel can be useful. They may keep their own domain databases, but
once context is ingested into the kernel, the kernel must be able to explore it
as a context database.

The kernel should behave like:

```text
write context
index context
query by anchor, scope, dimension, time, relation, provenance
return answerable context, evidence, path, and position
```

The kernel should not behave like:

```text
dump a flat bundle of text and force the caller to understand the storage graph
```

## Public Promise

The honest public promise is:

```text
If you ingest context with anchors, scopes, entries, relations, timestamps, and
details, the kernel can later recover where an answer came from, what happened
before or after it, and which path or evidence supports it.
```

The promise is not:

```text
The kernel magically understands every application database.
The kernel solves memory by vector search alone.
The kernel answers every question without an LLM.
```

Basic traversal should be deterministic CPU/I/O. LLMs remain useful for
relationship extraction, summarization, reranking, and final answer generation.

## Kernel Vocabulary

`session` is not a kernel-level concept.

The kernel-level concept is:

```text
scope
```

A scope is any bounded region of context that can be traversed. An application
may call that region a session, conversation, attempt, incident phase, workflow
run, tool-call group, benchmark haystack segment, or deployment window. Those
names belong to the application layer.

Canonical kernel vocabulary:

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

Application vocabulary is mapped into kernel vocabulary at ingestion time:

```text
application "session"       -> kernel scope with dimension="conversation"
application "turn"          -> kernel entry with sequence index
application "attempt"       -> kernel scope with dimension="attempt"
application "incident step" -> kernel entry with dimension="incident"
```

This keeps the kernel agnostic while still allowing precise traversal.

## First Public Slice

The first public slice should be intentionally small:

1. Define the `anchor/scope/entry/relation/detail/provenance` contract.
2. Add JSON schemas and examples for conversation, incident, workflow, and
   benchmark-like records.
3. Add MCP tools that call Kernel 1.0 application services instead of
   duplicating traversal logic.
4. Prove temporal and multi-scope retrieval with one deterministic demo.
5. Publish the limitation: this is traversal and evidence recovery, not a claim
   that every benchmark category is solved.

The demo should answer questions like:

```text
Was this statement before or after the earlier one?
Which later entry superseded the previous fact?
Which entries across scopes support this answer?
```

And it should return:

- the answer;
- the scope and entry position;
- the timestamp/order;
- the path when a semantic path exists;
- the evidence text;
- warnings when context is missing or ambiguous.

## Design Principles

### 1. Domain Agnostic, Not Traversal Blind

The kernel should not hard-code LongMemEval, PIR, incident management, or any
single application schema.

It should understand generic traversal coordinates:

```text
where is this context anchored?
which scope contains this entry?
what happened before or after this entry?
which relation path proves this answer?
which detail payload supports this fact?
which source produced it?
```

### 2. One Mental Model For Ingestion And Reading

The same concepts must round-trip:

```text
ingest anchor          -> read anchor
ingest scope           -> read scope
ingest entry           -> read entry
ingest relation        -> read path
ingest detail          -> read evidence
ingest timestamp/order -> read before/after position
```

The caller should not learn one model for writing and another model for reading.

### 3. Simple Public API

Most callers should only need:

```text
kernel_ingest_context
kernel_explore_context
kernel_get_context
kernel_inspect_context
```

Lower-level tools can exist for advanced/debug use, but the common path must be
small and stable.

### 4. CPU/I/O By Default

Graph traversal is not a GPU workload.

Traversal by anchor, dimension, scope, timestamp, sequence, relation type, or
provenance path is a CPU and storage/index workload, like querying a database.

GPU is only needed when the flow explicitly asks for model inference:

- relation extraction;
- evidence atom extraction;
- summarization;
- semantic reranking;
- final generative answer.

Basic context exploration should be cheap and scalable without GPU when the
graph has the required structure and indexes.

### 5. MCP Is A First-Class Protocol

Existing event and gRPC paths remain valid.

MCP becomes a first-class interface for:

- agents;
- IDE/tool clients;
- eval harnesses;
- demos;
- production tools;
- cross-project inspection.

The MCP server should call the same kernel application services used by gRPC. It
must not duplicate kernel logic.

## Required Traversal Capabilities

### Temporal Traversal

The kernel must answer:

```text
what happened before this?
what happened after this?
what was known at this time?
which fact is latest before this timestamp?
which later fact supersedes this earlier fact?
```

Supported forms:

```text
n entries before
n entries after
between timestamps
before timestamp
after timestamp
latest before timestamp
latest after timestamp
```

### Dimensional Traversal

The kernel must traverse by application-neutral dimensions:

```text
scope
conversation
action
attempt
incident
workflow
question
tool_call
deployment
agent
benchmark_record
```

If an application calls a grouping a "session", that term stays outside the
kernel contract and is mapped to a kernel scope plus a dimension.

### Semantic Traversal

The kernel must traverse by relation type and semantic class.

Examples:

```text
EVIDENCE_FOR_ANSWER
AGGREGATES_WITH
UPDATES_PREVIOUS_FACT
SUPERSEDES
TEMPORAL_CONTEXT
DISAMBIGUATING_CONTEXT
CAUSED_BY
MITIGATED_BY
RETRIED_AS
NEXT_ENTRY
HAS_SCOPE
```

### Raw Inspection Traversal

Semantic traversal is not enough. Debugging and audit require raw inspection.

Raw context should be traversable without polluting normal semantic reads:

```text
raw_context -> scope -> entry -> next entry
incident_raw -> alert -> log excerpt -> metric sample
workflow_raw -> action -> tool call -> result
```

## Conversation Example

A caller should be able to ask:

```text
For this graph, use all available dimensions and tell me whether this data point
was said before or after the referenced conversation. Identify the scope, entry,
timestamp, and path that prove it.
```

Expected answer shape:

```json
{
  "answer": "The datum was said after the earlier Chicago statement.",
  "position": {
    "temporal": "after",
    "scope_id": "answer_0b1a0942_2",
    "entry_index": 2,
    "timestamp": "2023-05-27T04:45:00Z"
  },
  "path": [
    {
      "type": "UPDATES_PREVIOUS_FACT",
      "from": "scope:answer_0b1a0942_1:entry:4",
      "to": "scope:answer_0b1a0942_2:entry:2"
    }
  ],
  "evidence": [
    "She moved to Chicago.",
    "My friend Rachel actually just moved back to the suburbs again."
  ],
  "confidence": "high"
}
```

The important behavior is not the final text alone. The kernel must explain
where the datum lives, whether it is before or after another datum, and which
scope/dimension/path proves it.

## Simple Ingestion Contract

Primary write tool:

```text
kernel_ingest_context
```

Minimal input:

```json
{
  "anchor": {
    "id": "question:830ce83f",
    "dimension": "question",
    "timestamp": "2023-06-13T15:15:00Z",
    "text": "Where did Rachel move to after her recent relocation?"
  },
  "scopes": [
    {
      "id": "answer_0b1a0942_1",
      "dimension": "conversation",
      "timestamp": "2023-05-24T22:23:00Z",
      "entries": [
        {
          "index": 4,
          "role": "user",
          "timestamp": "2023-05-24T22:23:00Z",
          "text": "She moved to Chicago."
        }
      ]
    }
  ],
  "relations": []
}
```

Optional semantic relation:

```json
{
  "from": "scope:answer_0b1a0942_1:entry:4",
  "to": "scope:answer_0b1a0942_2:entry:2",
  "type": "UPDATES_PREVIOUS_FACT",
  "class": "evidential",
  "evidence": "The later statement updates the earlier location."
}
```

Ingestion defaults:

- preserve raw order inside each scope;
- assign `entry.index` when omitted;
- infer `NEXT_ENTRY` order inside a scope;
- preserve full text as detail payload;
- index anchor id, scope id, dimension, timestamp, sequence, and relation type;
- accept optional semantic relations;
- allow later enrichment to add relations after raw ingestion.

### Ingestion Modes

Raw mode:

- caller provides scopes and entries only;
- kernel stores traversable raw context;
- use for transcripts, logs, histories, and benchmark haystacks.

Semantic mode:

- caller provides relations and selected evidence;
- kernel stores sparse, answer-oriented graph paths;
- use when an app or LLM already computed relevant relations.

Hybrid mode:

- caller provides raw scopes plus selected semantic relations;
- kernel keeps auditability while normal reads stay semantic and scoped.

## Simple Reading Contract

Primary read tool:

```text
kernel_explore_context
```

Minimal input:

```json
{
  "anchor": "question:830ce83f",
  "ask": "Where did Rachel move after her recent relocation?"
}
```

The kernel should decide by default:

- which dimensions to inspect;
- how much temporal context to include;
- which relation paths are relevant;
- whether raw neighboring entries are useful;
- whether the answer needs aggregation, update resolution, or temporal ordering.

Optional constraints:

```json
{
  "anchor": "question:830ce83f",
  "ask": "Was the suburbs statement before or after the Chicago statement?",
  "dimensions": "all",
  "include_path": true,
  "include_evidence": true,
  "token_budget": 4096
}
```

Expected output:

```json
{
  "answer": "The suburbs statement was after the Chicago statement.",
  "position": {
    "temporal": "after",
    "scope_id": "answer_0b1a0942_2",
    "entry_index": 2,
    "timestamp": "2023-05-27T04:45:00Z"
  },
  "path": [],
  "evidence": [],
  "confidence": "high",
  "warnings": []
}
```

Reading defaults:

- `dimensions="all"` unless restricted;
- semantic path first;
- raw neighboring entries only when useful or requested;
- bounded output by token budget;
- current/superseding facts preferred when update relations exist;
- ambiguity surfaced explicitly when facts conflict.

## Public MCP/API Surface

### `kernel_ingest_context`

Simple context ingestion.

Input:

```json
{
  "anchor": {},
  "scopes": [],
  "relations": []
}
```

Output:

```json
{
  "anchor_id": "question:830ce83f",
  "ingested": {
    "scopes": 2,
    "entries": 10,
    "relations": 3
  },
  "warnings": []
}
```

### `kernel_explore_context`

Default exploration tool. Use this when the caller has a question or wants a
position/provenance answer.

Input:

```json
{
  "anchor": "question:830ce83f",
  "ask": "Where did Rachel move after her recent relocation?",
  "dimensions": "all",
  "token_budget": 4096
}
```

### `kernel_get_context`

Simple retrieval tool. Use when the caller already knows the anchor and wants a
context bundle.

Input:

```json
{
  "anchor": "incident:checkout-latency-2026-04-30",
  "dimensions": "all",
  "token_budget": 4096
}
```

### `kernel_inspect_context`

Debug/audit tool for an anchor, scope, entry, relation path, node id, or
correlation id.

Input:

```json
{
  "id": "question:830ce83f",
  "include_raw": true
}
```

### Advanced Tools

Advanced/debug tools may exist, but should not be the normal caller path:

```text
kernel_get_context_window
kernel_get_context_path
kernel_rehydrate_scope
kernel_search_context
kernel_publish_graph_batch
```

## Storage And Indexing Requirements

Minimum indexed coordinates:

```text
node_id
anchor_id
root_node_id
correlation_id
causation_id
scope_id
dimension
dimension_id
entry_index
sequence
timestamp / observed_at / occurred_at
relation_type
semantic_class
source_agent
source_kind
```

These are coordinates, not domain coupling. They let the kernel traverse context
without knowing application meaning.

## Architecture Direction

MCP should be an adapter over kernel application services.

Recommended shape:

```text
client / agent / eval harness
        |
        v
kernel MCP server
        |
        v
kernel application ports
        |
        +--> gRPC transport
        +--> event ingestion
        +--> graph/query services
        +--> detail store
```

Events and gRPC remain supported. MCP is added as the simple tool-facing
protocol.

## Compute Model

Basic graph exploration should be CPU/I/O:

```text
index lookup
graph traversal
relation filtering
time-window scan
scope/entry ordering
path reconstruction
detail fetch
```

This is database-like work. It should not require GPU.

GPU or external model API is only required for explicit inference:

```text
relation extraction
evidence atom extraction
summarization
semantic reranking
generative final answer
```

Operational expectation:

```text
explore/read graph path -> CPU/I/O
generate/enrich/reason with LLM -> GPU or external model API
```

## Underpass Adoption

All Underpass projects should treat the kernel as the shared context layer.

Migration direction:

1. Existing event and gRPC paths remain supported.
2. New interactive, agentic, and evaluation clients should prefer MCP.
3. Projects should stop assuming context reconstruction must happen from their
   own local databases.
4. Project-specific schemas remain valid, but must map into kernel anchor,
   scope, entry, dimension, relation, detail, and provenance.
5. Cross-project inspection should use the same MCP tools.

Affected projects/classes:

- PIR;
- runtime/orchestrator services;
- LLM/vLLM services;
- evaluation harnesses;
- production incident tooling;
- demos;
- benchmark adapters;
- future Underpass apps that need memory, inspection, or rehydration.

## Implementation Plan

### Phase 0: Public Alignment

Deliver:

- this action plan in public docs;
- a short README/index link so readers can find it;
- explicit non-goals and current limitations;
- a tracked issue/PR checklist for the first implementation slice.

Exit criteria:

- the plan says what exists today and what does not;
- the plan does not imply benchmark success before the traversal API exists;
- the plan keeps Kernel 1.0 architecture as the base.

### Phase 1: Contract

Deliver:

- RFC/ADR for the traversal contract;
- schemas for `kernel_ingest_context`, `kernel_explore_context`,
  `kernel_get_context`, and `kernel_inspect_context`;
- examples for conversation, incident, workflow, and benchmark records;
- migration note for legacy API names that still say "session"; new kernel
  vocabulary should use "scope".

### Phase 2: Kernel Query Capabilities

Deliver application services for:

- context by anchor;
- context by dimension;
- context window by time;
- context window by sequence;
- context path by relation type;
- scoped raw inspection.

Also deliver:

- indexes;
- unit tests;
- integration tests against Neo4j/Valkey.

### Phase 3: MCP Server

Expose:

- `kernel_ingest_context`;
- `kernel_explore_context`;
- `kernel_get_context`;
- `kernel_inspect_context`;
- advanced/debug tools only where needed.

Also deliver:

- MCP service binary or crate;
- local configuration;
- Kubernetes deployment option;
- auth/TLS story;
- examples for Claude/Codex/agents.

MVP cut:

- local stdio MCP first;
- auth/TLS/Kubernetes after local behavior is proven;
- read/explore tools can ship before full write-side ingestion if they wrap
  existing gRPC/query services cleanly.

### Phase 4: Underpass Adoption

Deliver:

- PIR context inspection/retrieval via MCP;
- eval harness access via MCP;
- production incident context retrieval via MCP;
- demos for temporal and dimensional traversal;
- documentation in affected projects.

### Phase 5: Evaluation

Evaluate against:

- conversation multi-scope aggregation;
- knowledge update / supersession;
- temporal ordering;
- production incident reconstruction;
- agent attempt/retry reconstruction;
- LongMemEval balanced subsets.

Success criteria:

```text
single-scope recall remains high
multi-scope aggregation improves materially
knowledge-update chooses newest/superseding facts
incident reconstruction can traverse before/after deployment/action/attempt
MCP clients can retrieve the same context as gRPC clients
basic traversal remains CPU/I/O, not GPU-bound
```

## Experimental Evidence

This section records why the capability is needed. It should not define the
kernel contract.

### LongMemEval Pilot

The LongMemEval adapter exposed the need for multi-scope traversal.

Observed balanced 40-question subset:

| Run | Accuracy | single bounded context | multi-scope | temporal | update |
| --- | ---: | ---: | ---: | ---: | ---: |
| semantic-repair-balanced40-v2 | 25/40 = 0.625 | 10/10 | 3/10 | 7/10 | 5/10 |
| semantic-retrieval-balanced40-v3 | 26/40 = 0.650 | 10/10 | 2/10 | 7/10 | 7/10 |

Interpretation:

- A single bounded conversation works well.
- Update questions improve when retrieval is more aware of recency.
- Multi-scope aggregation remains weak without first-class evidence collection
  and traversal.
- The kernel currently retrieves what has been connected semantically; it does
  not yet provide enough generic exploration to recover missing context paths by
  itself.

### Concrete Failure Shape

For questions such as:

```text
How many items do I need to pick up or return?
```

The answer requires several evidence atoms across several scopes. A flat context
bundle or one semantic edge is not enough. The kernel must be able to collect
and return each evidence atom with provenance.

For questions such as:

```text
Where did Rachel move after her recent relocation?
```

The answer requires knowing that a later statement supersedes an earlier one.
The kernel must be able to traverse update/supersession paths and prefer the
current fact when the path proves it.

## Non-Goals

The kernel should not hard-code:

- LongMemEval logic;
- PIR logic;
- incident business rules;
- app-specific schemas as mandatory global schema.

Applications map their domain into kernel coordinates. The kernel provides
generic ingestion, traversal, inspection, and provenance.

## Decision

The kernel remains domain-agnostic, but becomes traversal-aware.

Events and gRPC remain valid transports.

MCP becomes the simple protocol for context ingestion, exploration, and
inspection across Underpass.

The first public step should be narrow and verifiable: make Kernel 1.0 expose
context traversal as a simple API, then measure the benchmark again against that
capability. Do not claim the benchmark is solved before the traversal contract
and MCP tools exist.

The principle to implement:

```text
Any context ingested by the kernel must be recoverable by time, dimension,
relation path, and raw inspection scope through a simple API.
```
