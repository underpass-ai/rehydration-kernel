# Inference Prompt Examples

The rehydration kernel is LLM-agnostic. It renders structured context via
gRPC (`GetContext`, `GetContextPath`, `RehydrateSession`) and returns text
that any LLM can consume.

These example prompts show how to feed rehydrated context into an LLM for
common agentic tasks. They are **not** part of the kernel contract — adapt
them to your model, domain, and task.

## How the kernel renders context

The `RenderedContext` response contains:

- `content` — flat text with salience-ordered sections (root, focus,
  relationships by semantic class, node details)
- `sections[]` — keyed sections for selective consumption
- `tiers[]` — multi-resolution tiers:
  - **L0 Summary** — objective, status, blocker, next action (~100 tokens)
  - **L1 Causal Spine** — root + focus + explanatory relations (~500 tokens)
  - **L2 Evidence Pack** — structural relations, neighbors, details (remaining)

Choose the level of detail based on your token budget and task.

## Prompt examples

### 1. Failure diagnosis (incident response)

Use case: an agent needs to identify what went wrong and where to restart.

```
api/examples/inference-prompts/failure-diagnosis.txt
```

### 2. Implementation rationale (code review, audit)

Use case: an agent needs to explain why a task was implemented a specific way.

```
api/examples/inference-prompts/implementation-rationale.txt
```

### 3. Handoff resume (shift change, agent swap)

Use case: a new agent picks up work from a previous agent mid-task.

```
api/examples/inference-prompts/handoff-resume.txt
```

### 4. Constraint-aware planning (safety, compliance)

Use case: an agent plans under binding constraints that must be preserved.

```
api/examples/inference-prompts/constraint-planning.txt
```

## Tips

- **Quote rationale**: instruct the LLM to cite `rationale` fields from
  relationships, not just node titles. This is the kernel's main value.
- **Use tiers under budget pressure**: L0+L1 gives the causal spine in
  ~600 tokens. Feed only those tiers when the full context is too large.
- **Structured JSON output**: ask for JSON responses when you need to
  parse the LLM output programmatically.
- **Semantic class hints**: mention that relationships have `semantic_class`
  (causal, motivational, evidential, etc.) so the LLM knows which
  relationships carry explanatory weight.
