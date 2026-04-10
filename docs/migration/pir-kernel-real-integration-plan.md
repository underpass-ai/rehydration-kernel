# PIR Kernel Real Integration Plan

Status: Active execution plan  
Scope: slices required before wiring the real `PIR` runtime to the kernel

## Intent

Turn the current `PIR -> kernel` guidance into a practical execution plan with
clear slices, evidence, and exit criteria.

This document complements, but does not replace:

- [`pir-kernel-integration-reference.md`](pir-kernel-integration-reference.md)

The reference defines the contract. This plan defines the order in which we
prove that the contract is good enough for the real integration.

## What We Are Validating First

Current phase:

- the kernel contract is usable by a model-driven producer
- the kernel can rehydrate and return useful context for a PIR consumer
- the `semantic_class` path is stable enough when assisted by the reranker

Current phase is **not** trying to prove:

- that the primary model can diagnose incidents autonomously
- that the primary model can choose the mitigation without strong guidance
- that the model can solve incidents end-to-end without fixture support

That distinction matters. We first validate the contract, then the autonomy of
the model using that contract.

## Questions This Plan Must Answer

For the real PIR integration, we need practical evidence for four questions:

1. Can PIR publish graph updates into the kernel with stable identity and
   predictable retries?
2. Can PIR read the materialized graph back through `GetContext` and
   `GetNodeDetail`?
3. Can an LLM consume `rendered.content` from the kernel and answer correctly
   from rehydrated context?
4. Can the model write nodes and relations in a shape that the kernel accepts,
   first with strong fixtures and later with weaker guidance?

## How This Plan Helps PIR Design Iteration

This plan is not only about pass/fail validation. It is also the feedback loop
for improving the design of `PIR`.

Each slice should tell us something concrete about the `PIR` design:

- whether `PIR` should emit one wave or multiple waves for the same incident
- which facts belong in nodes, relations, and `node_details`
- whether `PIR` should keep deterministic node ids for findings, tasks, and
  decisions
- whether `PIR` should rely on the semantic reranker in production
- which `GetContext` shape gives the best downstream answers for PIR
- whether `reason_preserving` is the right default read mode for PIR

The intended design loop is:

1. run a slice
2. inspect the artifacts
3. identify where the contract is fine but the `PIR` representation is weak
4. adjust the `PIR` design
5. rerun the same slice

The most important `PIR` design knobs to iterate are:

- incident identity strategy
- wave boundaries and retry policy
- node taxonomy: incident, finding, decision, task, evidence, constraint
- relation semantics and support fields
- what gets rendered into `node_details` versus short summaries
- query profile: role, scopes, depth, token budget, and rehydration mode
- whether semantic reranking is mandatory or advisory

Each slice below therefore has two outputs:

- a validation result
- a design implication for the next PIR revision

## Slice Plan

### Slice 1. Live PIR incremental roundtrip + consumption

Goal:

- prove the full PIR contract on a real cluster with two materialization waves

Shape:

- wave 1 publishes a stable incident root plus initial findings and decisions
- wave 2 reuses the same incident identity, expands the graph, and rehydrates
  the enriched context
- both waves use distinct `run_id` values
- both waves use the semantic reranker before publish
- the second-wave `rendered.content` is passed back to the LLM

Evidence required:

- published message counts
- neighbor, relationship, and detail counts after each wave
- selected detail excerpts
- final LLM answer from `rendered.content`

Exit criteria:

- same incident root across both waves
- larger graph after wave 2
- reranker invoked for both waves
- LLM answer cites second-wave finding and second-wave task from rehydrated
  context

Design implication:

- tells us whether PIR should model incident evolution as incremental waves
  instead of one large overwrite
- tells us whether the current PIR graph shape survives the full publish and
  read loop without losing explanatory value

### Slice 2. Practical evidence report

Goal:

- answer the core questions with concrete artifacts instead of only `test passed`

Deliverables:

- one short run report
- raw publish/query summaries
- the final rendered excerpt used for LLM consumption
- the final LLM answer
- a short interpretation of what this proves and what it does not prove

Exit criteria:

- another engineer can inspect the report and understand what the contract
  enabled in practice

Design implication:

- gives the PIR team a concrete basis for changing prompts, ids, and graph
  shape without hand-waving

### Slice 3. Blind extraction fixture

Goal:

- reduce prompt leakage and stop handing the model the solution too explicitly

Current issue:

- the strong extraction fixtures already contain confirmed findings,
  deterministic node ids, and mitigation choices

Planned change:

- create a weaker extraction fixture with symptoms, evidence, and constraints
- remove explicit `confirmed finding` and `mitigation decision` wording
- keep the `GraphBatch` contract and schema, but loosen the content hints

Exit criteria:

- model still emits a valid bounded `GraphBatch`
- graph remains local, connected, and parsable

Design implication:

- shows how much of the current success comes from the contract versus how much
  comes from over-specified PIR prompting

### Slice 4. Structural extraction evaluation

Goal:

- evaluate whether the model writes kernel-friendly graph structure before we
  ask whether it "solved" the incident

Checks:

- required root node present
- key finding and action nodes present
- required relations present
- graph connected from the root
- details attached to the right nodes
- `semantic_class` before reranker
- `semantic_class` after reranker

Exit criteria:

- we can quantify where the primary model is sufficient and where the reranker
  is still necessary

Design implication:

- tells us whether PIR should depend on the reranker by default
- tells us which node kinds or relation types need tighter prompt guidance or
  local repair logic

### Slice 5. Blind context consumption

Goal:

- test whether the model can understand kernel context without relying on
  over-specified extraction fixtures

Shape:

- publish blind extraction output
- rehydrate with `reason_preserving`
- ask non-literal questions about the incident
- reject answers that only copy obvious surface text without integrating the
  causal path

Exit criteria:

- the answer uses the rehydrated context correctly
- the answer does not invent absent causes or mitigations

Design implication:

- tells us whether PIR is asking the kernel for the right context shape
- tells us whether PIR needs different scopes, depth, or read modes for
  downstream reasoning

### Slice 6. Real PIR adapter start

Goal:

- move from test fixtures to the first real PIR-produced `GraphBatch`

Minimum adapter responsibilities:

- obtain a `GraphBatch`
- validate locally
- translate to projection events
- publish to NATS
- query `GetContext` and `GetNodeDetail`

Exit criteria:

- one real PIR-produced incident can be materialized and read back using the
  same contract and retry rules proven in earlier slices

Design implication:

- marks the point where PIR iteration moves from fixture design to runtime
  adapter design

## Execution Order

1. Finish Slice 1 and keep the artifacts.
2. Write the Slice 2 evidence report.
3. Build the blind extraction fixture.
4. Add structural evaluation for blind extraction.
5. Add blind context-consumption evaluation.
6. Start the real PIR adapter against the already-proven contract.

## Suggested Success Matrix

| Capability | Strong fixture | Blind fixture | Real PIR |
|:-----------|:---------------|:--------------|:---------|
| Publish to kernel | required | required | required |
| Read rehydrated context | required | required | required |
| Consume `rendered.content` | required | required | required |
| Correct node/relation structure | required | required | required |
| Correct `semantic_class` without reranker | optional | measured | measured |
| Autonomous diagnosis | out of scope | exploratory | exploratory |
| Autonomous mitigation choice | out of scope | exploratory | exploratory |

## Per-Slice Output Template

For each slice, capture these seven items:

1. objective
2. exact input used
3. exact output produced
4. pass/fail result
5. what this proves
6. what this does not prove
7. PIR design change, if any

## Immediate Next Step

Run Slice 1 as the first full live proof:

- two PIR waves
- stable incident identity
- reranker on both waves
- kernel rehydration
- `rendered.content` consumed by the LLM

If Slice 1 passes, the next work should not jump straight to "real PIR". The
next correct move is Slice 2 plus the blind extraction fixture, so we know
which part of the result comes from the contract and which part comes from
over-specified prompts.
