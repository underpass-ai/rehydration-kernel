# Roadmap To State Of The Art

This roadmap positions `rehydration-kernel` as a systems artifact for
explanatory graph context rehydration in agentic workflows.

The current repository already demonstrates a strong core claim:

- typed relationship explanation can be preserved end-to-end
- that explanation materially improves diagnosis and recovery over
  structural-only edges
- explanation can survive bounded retrieval better than node detail alone

That is publishable. It is not yet enough to claim state of the art across the
broader fields of agent memory, graph retrieval, or graph-native reasoning.

The gap is not primarily conceptual. The gap is empirical:

- too few external baselines
- graphs are still small and synthetic
- limited runtime and domain diversity
- no public benchmark or dataset

This document splits the work into three levels:

- `Level 1`: submission-ready paper
- `Level 2`: strong paper with hard-to-dismiss evidence
- `Level 3`: state-of-the-art push

## Current Position

Today the artifact already supports:

- typed explanatory relationships with `semantic_class`, `rationale`,
  `motivation`, `method`, `decision_id`, `caused_by_node_id`, and `sequence`
- end-to-end preservation across event ingestion, projection, query, and
  rendered context
- operational use cases for:
  - failure diagnosis and rehydration-point recovery
  - why-was-this-implemented-like-this reconstruction
  - interrupted handoff and resumable execution
  - dominant-reason preservation under token pressure
- reproducible paper harness under
  `scripts/ci/integration-paper-use-cases.sh`

This is best described as:

`near-frontier systems work on explanatory context rehydration`

It is not yet:

`state-of-the-art agent memory or graph reasoning`

## Level 1: Submission-Ready

Goal:

Ship a paper that is technically coherent, reproducible, and reviewable
without overclaiming.

### Required Work

- ~~Freeze the current paper artifact~~ (done)
- ~~Keep the current four use cases stable~~ (done: UC1-UC4)
- ~~Add one denser `meso` graph per use case family~~ (done: UC1 meso variant)
  - remaining: expand meso to UC2-UC4 and target `20-40` nodes
- ~~Add one additional baseline beyond structural-only~~ (done: `detail_only`)
- Add latency capture to the same artifact:
  - query latency
  - render latency
  - total end-to-end latency
- ~~Unify wording across paper and repo~~ (done)

### Metrics Required

- ~~`explanation_roundtrip_fidelity`~~ (done)
- ~~`detail_roundtrip_fidelity`~~ (done)
- ~~`causal_reconstruction_score`~~ (done)
- ~~`rehydration_point_hit`~~ (done)
- ~~`dominant_reason_hit`~~ (done)
- ~~`rendered_token_count`~~ (done)
- ~~`retry_success_hit`~~ (done)
- ~~`retry_success_rate`~~ (done)
- `latency_ms` (remaining)

### Exit Criteria

- ~~all paper-use-case tests pass in CI~~ (done)
- ~~one dense noisy graph is included in the artifact~~ (done: UC1 meso)
- ~~at least one non-structural baseline is reported~~ (done: detail-only)
- ~~the paper only makes bounded systems claims~~ (done)

### Claim You Can Defend

`A reusable kernel can preserve explanatory graph context across runtime boundaries, and that preserved explanation materially improves diagnosis, handoff, and bounded retrieval over structural context alone.`

## Level 2: Strong Paper

Goal:

Move from a good artifact paper to a strong systems paper with comparative
evidence.

### Required Work

- Add `closed-loop recovery` evaluation:
  - detect bad relationship
  - select rehydration point
  - retry from that point
  - verify corrected outcome
- Expand seeds into three scales:
  - `micro`: `5-8` nodes
  - `meso`: `20-40` nodes
  - `stress`: `80-150` nodes
- Add noise controls:
  - distractor decisions
  - irrelevant evidence
  - competing motivations
  - superseded revisions
  - sibling tasks
- Add temporal and revision behavior:
  - decision `v1/v2`
  - detail revisions
  - retry chains
  - superseded edges
- Evaluate both pull and event-driven flows with the same metrics
- Add at least two external baseline families:
  - plain linear context or flat RAG
  - structural graph retrieval
  - GraphRAG-like compressed graph summary if practical
- Add at least one second domain:
  - operations / incident response
  - software implementation / debugging
  - infrastructure change management

### Metrics Required

- all Level 1 metrics
- `retry_success_rate`
- `wrong_branch_rate`
- `selected_nodes`
- `selected_relationships`
- `token_efficiency`
- `cross_runtime_consistency`

### Exit Criteria

- explanatory kernel beats all internal baselines on recovery and explanation
- recovery is validated by corrected retry, not only path selection
- results hold under noisy meso graphs
- same pattern appears in at least two domains

### Claim You Can Defend

`Explanatory relation preservation is not only descriptive; it improves restartable execution and reason-preserving retrieval across agent runtimes, domains, and noisy graph neighborhoods.`

## Level 3: State-Of-The-Art Push

Goal:

Compete seriously with the best current agent-memory and graph-context systems
literature.

### Required Work

- Release a public benchmark:
  - graph seeds
  - ground truth traces
  - retry labels
  - distractor branches
  - token-budget slices
- Build a benchmark harness others can run without repo-specific knowledge
- Compare against external systems or credible approximations:
  - plain RAG
  - GraphRAG
  - hyper-relational graph retrieval
  - temporal agent-memory graphs
- Add human or expert evaluation for explanation usefulness:
  - was the restart point sensible
  - was the dominant reason preserved
  - was the explanation audit-friendly
- Add temporal memory as a first-class benchmark dimension:
  - stale reasons
  - changed decisions
  - resumed work across sessions
  - long-lived revision chains
- Add multi-agent handoff benchmarks:
  - planner to executor
  - executor to recovery agent
  - recovery agent to verifier
- Evaluate under realistic scale:
  - `100+` nodes per local graph
  - `1000+` nodes total per scenario when possible
- Add learned or heuristic ranking experiments:
  - edge salience
  - explanation-first pruning
  - sequence-aware retrieval

### Metrics Required

- all Level 1 and Level 2 metrics
- `benchmark win rate`
- `human preference rate`
- `temporal consistency score`
- `handoff success rate`
- `auditability score`
- `cost_per_successful_recovery`

### Exit Criteria

- public benchmark is released
- external baselines are reproduced or approximated fairly
- explanatory kernel wins on key operational metrics, not only format fidelity
- results generalize across domains, runtimes, and graph scales

### Claim You Can Defend

`Explanatory graph context rehydration is a competitive state-of-the-art approach for operational agent memory when the task requires diagnosis, restartable execution, handoff continuity, and reason preservation under bounded context.`

## Workstreams

The implementation work can be split into five parallel tracks.

### Workstream A: Data Scale

- convert current hand-written seeds into parameterized generators
- support:
  - `chain_length`
  - `noise_branches`
  - `distractor_ratio`
  - `revision_count`
  - `handoff_count`
  - `detail_size_words`
- keep deterministic `micro` fixtures for exact assertions
- add generated `meso` and `stress` fixtures for benchmark-style runs

### Workstream B: Recovery Loop

- add corrected-retry tasks to `UC1`
- add resumed-task completion to `UC3`
- score whether the chosen rehydration point actually improves execution

### Workstream C: Baselines

- structural-only
- detail-only
- root-only
- flat chronological trace
- compressed graph summary

### Workstream D: Runtime Breadth

- unify pull and event-driven metrics
- add one more runtime surface beyond the current local harness if practical
- measure consistency of selected point and preserved reason across runtimes

### Workstream E: Publication Packaging

- lock artifact version
- publish benchmark format
- document reproduction path
- prepare arXiv package and ARR submission package

## Recommended Order

If the goal is to publish fast without drifting:

1. finish `Level 1`
2. add `meso` noisy graphs
3. implement `closed-loop recovery`
4. add one external baseline family
5. then decide whether to stop at a strong systems paper or invest in
   `Level 3`

## Immediate Next Steps

- keep `UC1-UC4` as the stable micro suite
- add parameterized seed generators for `meso` graphs (UC2-UC4 still need meso)
- ~~add `detail_only` baseline~~ (done)
- ~~add `retry_success_rate` to UC1 and UC3~~ (done)
- add latency capture to the paper artifact
- expand meso variants beyond UC1

## Non-Goals For Now

Do not expand into these until Level 2 is stable:

- generic world-model claims
- autonomous causal inference from raw logs
- large-scale pretraining claims
- universal memory architecture claims across all agent types

The strongest line remains narrow and operational:

`preserving typed explanation in graph relationships makes rehydrated context more useful for restartable agent work`
