# PIR Fix Planning Next Session Handoff

Date: `2026-04-12`

Status: ready for next session

## Current Position

The important state at close of day is:

- `A0` remains the working baseline
- `D1` is informative but not yet promotable
- the latest valid `D1` live smoke failed truthfully, not operationally

Baseline evidence already in place:

- `A0` produced real `.completed` runs on deployed `PIR`
- `A0` passed the first repeatability batch
- `A0` passed the first widened YAML scenario batch
- delivery idempotency on `.failed` and `.escalated` is now closed
- LLM request and response tracing is persisted in PostgreSQL

Relevant documents:

- [`pir-fix-planning-a0-repeatability-report-2026-04-12.md`](pir-fix-planning-a0-repeatability-report-2026-04-12.md)
- [`pir-fix-planning-a0-scenario-matrix-report-2026-04-12.md`](pir-fix-planning-a0-scenario-matrix-report-2026-04-12.md)
- [`pir-fix-planning-d1-live-failure-analysis-2026-04-12.md`](pir-fix-planning-d1-live-failure-analysis-2026-04-12.md)
- [`pir-fix-planning-experiment-matrix.md`](pir-fix-planning-experiment-matrix.md)

## What The `D1` Failure Actually Means

The first valid `D1` run did not fail because of:

- deployment wiring
- NATS delivery
- TLS
- malformed JSON
- truncation
- timeout before completion

It failed because:

- the planner kept producing rollback plans that were too vague, risky, or
  operationally unconfirmed
- the reranker consistently showed strong alignment with `finding`
  but weak alignment with `evidence`
- the judge rejected all four internal attempts

Short version:

- `D1` added diagnostic signal
- `D1` did not improve acceptance yet

## Decision For Tomorrow

Do not replace `A0` with `D1`.

Treat `A0` as the operational baseline and `D1` as an experimental branch.

The next slice should improve the planner contract in ways that benefit both
`A0` and `D1`, then re-run `A0` before deciding whether to keep investing in
`D1`.

## Recommended Next Iteration

Work on the common planner-quality layer first:

1. strengthen the `fix_planning` output contract
2. add deterministic local validation for rollback quality
3. capture this `D1` failure as a regression fixture
4. re-run `A0`
5. only then re-run `D1`

## Concrete Work Items

### 1. Strengthen The Planner Schema

Extend `fix_planning` output expectations with more operational structure.

Candidate additions:

- `evidence_basis`
- `assumptions`
- `rollback_preconditions`
- `rollback_steps`
- `rollback_risks`

Goal:

- force the planner to state what is actually grounded in evidence
- make rollback operationally concrete instead of generic prose

### 2. Add Deterministic Local Rollback Validation

Before judge acceptance, reject rollback plans that:

- rely on unconfirmed admin endpoints
- rely on vague `restart service` language that does not revert the change
- propose actions that obviously worsen the incident shape
- omit concrete rollback steps

Goal:

- stop wasting retries on operationally weak rollback proposals

### 3. Create A Regression Fixture From The `D1` Failure

Use the stored `llm_traces` and scenario facts from:

- `incident_run_id = a38e2cb4-1d54-4f97-b6fb-2a128b57bd32`

Goal:

- preserve the exact failure pattern
- compare the next planner-contract iteration against a real failing case

### 4. Re-Test `A0` First

Do not jump directly back to `D1`.

Reason:

- the common planner-quality fixes should improve the baseline too
- if `A0` improves, that confirms the next bottleneck was planner contract, not
  lack of reranking

### 5. Re-Test `D1` Only After The Common Fixes

If `A0` remains strong after the common fixes, then re-run `D1` to see whether
the reranker now adds measurable value.

What we want to know then:

- does `D1` improve acceptance rate over the improved `A0`
- does it reduce retries
- does it improve evidence grounding in accepted outputs

## What Not To Do Tomorrow

Avoid these as the first move:

- switching planner model
- switching to `A1`
- adding more graph roles
- shrinking prompts just for speed
- treating `D1` as the new default

The data today does not justify those moves yet.

## Operational Notes

- `PIR` PostgreSQL has the full LLM `request_body` and `response_body` in
  `llm_traces`
- the migration `Job` now exists in the chart, but it requires a new image that
  actually contains `/pir-migrate`
- image `v0.4.16-d1-reranker-20260412T202020Z` does not contain
  `/pir-migrate`, so `migrationJob.enabled=true` is not valid with that image

## Transport Note: `Envoy` Is A Later Option, Not The Next Fix

`Envoy` is now explicitly noted as a possible later hardening layer for:

- outbound gRPC and HTTP retries
- circuit breaking
- quota and rate-shaping support
- uniform mTLS handling across runtime, kernel, and model endpoints

But it should not be treated as the next solution to the current live issue.

Reason:

- the current failure mode is a control-plane and consumer-topology problem
- a long-running `*.requested` event can delay a fast
  `*.completed|*.failed|*.escalated` event from another incident
- that is a FIFO and lane-separation problem, not primarily a transport proxy
  problem

So the next architectural move should still be:

1. separate `request` and `result` consumers
2. preserve FIFO semantics per `incident_run_id`
3. avoid global blocking between unrelated incidents

After that is stable, `Envoy` can be reconsidered as a transport and policy
layer, especially if retries, quotas, and mTLS behavior become complex enough
to justify it.

## First Command To Resume Tomorrow

Start by reopening these three docs in this order:

1. [`pir-fix-planning-next-session-handoff-2026-04-12.md`](pir-fix-planning-next-session-handoff-2026-04-12.md)
2. [`pir-fix-planning-d1-live-failure-analysis-2026-04-12.md`](pir-fix-planning-d1-live-failure-analysis-2026-04-12.md)
3. [`pir-fix-planning-experiment-matrix.md`](pir-fix-planning-experiment-matrix.md)
