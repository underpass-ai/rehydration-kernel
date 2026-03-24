# Draft Paper: Agentic Context Rehydration

Status: Working submission draft

## Working Title

Explanatory Graph Context Rehydration for Agentic Systems: Bounded Retrieval
Across Pull and Event-Driven Runtimes

## One-Sentence Thesis

`rehydration-kernel` can serve as a generic context engine for agents by
rehydrating bounded graph context from projection state, exposing it through
stable query contracts, and driving runtime actions without embedding
product-specific nouns into the kernel core.

## Why This Can Be A Paper

The repo already demonstrates a coherent systems claim:

- context is represented as a generic graph over root node, neighbor nodes,
  relationships, and extended node detail
- graph context can be rehydrated from projection storage instead of being
  assembled ad hoc inside an agent runtime
- the same context engine works for both pull-driven and event-driven agent
  flows
- the runtime boundary stays narrow and replaceable

That is enough for a credible systems or applied agentic paper if the claims
stay disciplined and the evaluation remains tied to observable behavior.

## Strong Claim We Can Defend Today

The strongest claim supported by the current repository is:

> A generic node-centric context rehydration kernel can decouple context
> management from agent runtimes while still supporting bounded retrieval and
> end-to-end runtime execution in both synchronous and asynchronous workflows.

This claim is already backed by code, contracts, and container-backed tests in
the repo.

## Strong Claim We Still Cannot Defend Yet

The infrastructure claim is now stronger than when this draft started.

The kernel already preserves typed relationship explanations end to end,
including:

- `semantic_class`
- `rationale`
- `motivation`
- `method`
- `decision_id`
- `caused_by_node_id`
- `evidence`
- `confidence`
- `sequence`

That means this claim is now defendable:

> Relationships can preserve why a downstream node exists and surface that
> explanation through event, storage, query, and render boundaries.

What is still not defendable without experiments is the stronger effectiveness
claim:

> Explanatory relationships improve agent diagnosis, rehydration-point
> selection, or task success over purely structural graph context.

That claim belongs to the evaluation section and must be supported by
ablations.

## Paper Positioning

This paper should be positioned between:

- GraphRAG style systems that retrieve structured context for LLMs
- event graph and hyper-relational knowledge graph work that preserves richer
  semantics than plain triples
- agent runtime papers that focus on tool execution but under-specify context
  retrieval

Our angle is different from standard RAG papers:

- we are not primarily ranking documents
- we are rehydrating operational graph state from projections
- we expose a reusable kernel boundary instead of coupling context assembly to
  one runtime or one product taxonomy

## Core Research Questions

1. Can a generic node-centric graph model support useful agent context without
   hardcoding product-specific workflow nouns?
2. Can context rehydration be isolated into a reusable kernel that serves
   multiple runtime entrypoints?
3. Does bounded graph rehydration preserve enough information for successful
   downstream runtime action?
4. What is lost when relationships remain non-causal and non-qualified?

## Proposed Contributions

1. A reusable kernel architecture for context rehydration over projected graph
   state.
2. A generic contract surface for graph retrieval, node detail retrieval, and
   rendered bounded context.
3. A reproducible evaluation harness covering pull-driven and event-driven
   agent execution.
4. A roadmap from simple binary relations to causal or hyper-relational context
   edges for stronger agent reasoning.

## Experimental Artifact In This Repo

The repo already contains the main artifact needed for the paper:

- a graph-native kernel implementation
- a gRPC query boundary
- an async projection boundary
- a runnable reference agent
- two runtime implementations
- container-backed integration fixtures

Relevant assets:

- pull-driven agentic flow:
  - `scripts/ci/integration-agentic-context.sh`
  - `crates/rehydration-transport-grpc/tests/agentic_integration.rs`
- event-driven agentic flow:
  - `scripts/ci/integration-agentic-event-context.sh`
  - `crates/rehydration-transport-grpc/tests/agentic_event_integration.rs`
- minimal generic seed scenario:
  - `crates/rehydration-transport-grpc/tests/support/generic_seed_data.rs`
- explanatory relation seed scenarios:
  - `crates/rehydration-transport-grpc/tests/support/explanatory_seed_data.rs`
- zoomed bounded-context behavior:
  - `crates/rehydration-transport-grpc/tests/kernel_full_journey_integration.rs`

## Evaluation Strategy

### Scenario A: Minimal Generic Agentic Graph

Use the existing `generic_seed_data` fixture as the smallest controlled graph:

- root workspace node
- one focus work item
- one dependency node
- one detailed node

This scenario is useful for:

- verifying that the agent can derive a useful node from graph structure
- measuring end-to-end success with minimal graph noise
- reproducing pull and event-driven workflows deterministically

### Scenario B: Rich Operational Graph

Use the `starship_e2e` scenario as the richer graph:

- incident root
- multiple decisions
- multiple tasks
- subsystem nodes
- explorer workstream, checklist, and artifact leaf
- multiple node details

This scenario is useful for:

- testing bounded context under denser graphs
- testing subtree zoom behavior
- evaluating whether the rendered context keeps the useful local neighborhood

## Representative Use Cases

These are the main use cases the explanatory relation model should support.
They are not the benchmark by themselves. They are the product-level stories
from which the evaluation should be derived.

### UC1. Failure Diagnosis And Rehydration-Point Recovery

Operational question:
Given a graph that implemented a task incorrectly, can we identify which
relationships were wrong and from which upstream node the agent should
rehydrate context to try again correctly?

What the kernel must provide:

- a failing downstream node such as a rejected artifact or failed validation
- explanatory relationships that preserve the producing decision or
  causal source
- enough path context to isolate suspect edges instead of re-reading the whole
  graph

Why this matters:

- this is the concrete recovery workflow for agentic systems
- it moves the kernel from passive retrieval to restartable execution support
- it directly tests whether relationships express why the next node happened

### UC2. Why-Was-This-Implemented-Like-This Analysis

Operational question:
Given a task that was implemented in a particular way, can we reconstruct the
reason for that implementation choice from graph context alone?

What the kernel must provide:

- the incoming explanatory edge to the task
- `rationale`, `motivation`, and decision linkage
- rendered context that preserves the same explanation without domain-specific
  custom code

Why this matters:

- it tests whether the graph captures not just structure but intent
- it makes decisions and tasks auditable after the fact
- it is the smallest defendable example of explanatory context rehydration

### Use Cases To Tests Mapping

The E2E suite should be presented as evidence for these use cases:

- UC1 maps to failure diagnosis, suspect-relationship isolation, and
  rehydration-point discovery tests
- UC2 maps to explanation reconstruction and rendered-context fidelity tests
- ablations then test whether explanatory relations outperform structural-only
  edges on the same use cases

## Experiments We Can Run Now

### E1. Cross-Runtime Pull Workflow

Question:
Can the same kernel context drive two different runtimes?

Method:

- run `agentic_integration.rs`
- compare `RecordingRuntime` and `UnderpassRuntimeClient`

Expected measurable outcome:

- both flows select the same expected node
- both flows write the expected artifact
- both flows include root title, focused title, and focused detail in output

### E2. Cross-Runtime Event-Driven Workflow

Question:
Can the same kernel context be consumed through an async trigger instead of a
pull-only caller?

Method:

- run `agentic_event_integration.rs`
- publish `context.bundle.generated`

Expected measurable outcome:

- the event trigger completes before timeout
- both runtimes produce the expected artifact
- the same agent logic is reused across sync and async entrypoints

### E3. Bounded Context Sensitivity

Question:
Does the kernel preserve utility under token limits?

Method:

- reuse the existing token-budget test coverage
- sweep token budgets over a small range
- compare rendered output size and task success

Existing evidence:

- the existing use-case and ablation suites already assert token-budget behavior

Expected measurable outcome:

- rendered token count decreases as budgets shrink
- task success stays stable until a threshold
- after that threshold, success drops in a measurable and explainable way

### E4. Failure Diagnosis And Rehydration Point Discovery

Question:
Given a graph that implemented a task incorrectly, can we identify which
relationships were wrong and which upstream node should be used as the
rehydration point?

Method:

- seed a failing chain such as `incident -> bad decision -> wrong task ->
  failed artifact`
- encode the failure evidence on the downstream relationship with
  `decision_id` and `caused_by_node_id`
- query `GetContextPath` to the failed artifact
- reconstruct the suspect relationships associated with the same producing
  decision

Expected measurable outcome:

- the failure evidence points back to a concrete rehydration node
- the set of suspect relationships is smaller than the full local graph
- the rendered context exposes enough explanation to restart from the correct
  upstream point

### E5. Why-Was-This-Implemented-Like-This Analysis

Question:
Can the kernel explain why a task was implemented in a specific way?

Method:

- seed a chain such as `incident -> decision -> task`
- attach `rationale`, `motivation`, and `decision_id` to the incoming task
  edge
- query `GetContextPath` to the task
- reconstruct the implementation reason from the relationship explanation and
  compare it with the rendered context

Expected measurable outcome:

- the incoming edge to the task provides a stable explanation of the chosen
  implementation
- the rendered context includes that explanation without custom domain code in
  the kernel

### E6. Local Zoom / Subgraph Rehydration

Question:
Can the kernel serve a local subgraph without dragging the whole root context?

Method:

- reuse the explorer workstream subtree checks in the full-journey tests

Existing evidence:

- the zoomed rendered context contains local workstream, checklist, and leaf
  detail while excluding the original root title and root detail

Expected measurable outcome:

- selected nodes and relationships shrink
- rendered context excludes unrelated root-level content
- local task evidence remains intact

### E7. Detail Ablation

Question:
How much does extended node detail contribute beyond graph neighborhood alone?

Method:

- rerun Scenario A and Scenario B while withholding detail materialization
- compare output correctness and action quality

Expected measurable outcome:

- tasks that need precise operational detail degrade first
- graph structure alone remains sufficient for coarse focus selection

### E8. Relationship Explanation Ablation

Question:
How much explanatory value comes from rich relation metadata beyond structural
edges alone?

Method:

- rerun the same scenario twice:
  - explanatory edges with rationale, motivation, method, and decision linkage
  - structural-only edges with ordering but without rich explanation fields
- compare rendered specificity, rehydration-point discovery, and diagnostic
  usefulness
- keep nodes and topology fixed so the difference comes from explanation
  richness only

Expected measurable outcome:

- rendered context becomes shorter and less specific without explanations
- diagnostic traces become harder to reconstruct
- rehydration-point discovery should degrade before raw topology fails

## Preliminary Results

The current paper harness produces direct evidence for four explanatory use
cases across multiple ablation variants:

- full explanatory relations with detail
- full explanatory relations without detail
- detail-only relations with detail
- structural-only relations with detail
- meso-scale (denser noisy graph) variant for UC1
- token-budget constrained variants (192 and 96 tokens) for UC4

Current results from `artifacts/paper-use-cases/summary.json` and
`artifacts/paper-use-cases/results.md` show a clean pattern.

### Result R1. Explanatory Relations Matter More Than Detail Removal

Across all four use cases (UC1-UC4):

- full explanatory context reaches `explanation_roundtrip_fidelity = 1.0`
- full explanatory context reaches `causal_reconstruction_score = 1.0`
- structural-only context drops to `causal_reconstruction_score = 0.143`
- detail-only context reaches `causal_reconstruction_score = 0.429`

Interpretation:

- removing explanation destroys most of the causal or motivational signal even
  when node detail is still present
- detail alone is not enough to reconstruct why the next node exists
- detail-only recovers more than structural-only but far less than explanatory

### Result R2. Detail Still Matters, But Less Than Explanatory Edges

Across UC1-UC3:

- explanatory-without-detail keeps `explanation_roundtrip_fidelity = 1.0`
- explanatory-without-detail drops to `causal_reconstruction_score = 0.857`

Interpretation:

- the relation explanation preserves the why-trace even without extended node
  detail
- detail still improves completeness, especially for failure diagnosis and
  recovery prompts

### Result R3. Token Reduction Tracks Loss Of Explanatory Power

Observed rendered token counts:

- UC1 full: `282`, structural-only: `200`, without-detail: `252`
- UC2 full: `285`, structural-only: `203`, without-detail: `255`
- UC3 full: `575`, structural-only: `374`, without-detail: `535`
- UC4 full: `223`, full@192: `175`, full@96: `89`, structural@96: `73`

Interpretation:

- structural-only edges make the prompt shorter, but that shorter prompt is
  materially worse for diagnosis and implementation-trace recovery
- removing detail gives a smaller token reduction and a smaller quality drop
- UC4 shows that even under token pressure (192 tokens), explanatory
  context preserves `1.0` causal reconstruction while structural at 96 tokens
  drops to `0.125`

### Result R4. Rehydration-Point Discovery Depends On Explanatory Linkage

For UC1 and UC3:

- full explanatory context finds the rehydration or continuation point correctly
- structural-only context loses recovery entirely
- detail-only context also loses recovery entirely
- explanatory-without-detail still recovers the correct upstream node

Interpretation:

- `decision_id` and `caused_by_node_id` are carrying the recovery signal
- this is direct evidence for the kernel value proposition in restartable
  agent workflows

### Result R5. Closed-Loop Retry Success Depends On Explanation

For UC1 and UC3:

- explanatory variants reach `retry_success_rate = 1.0` including no-detail
- structural-only and detail-only variants stay at `0.0`

Interpretation:

- detail-only context can preserve fragments of why-trace text but does not
  expose a stable machine-readable anchor for restart or resume
- the closed-loop signal is sharper than continuation-point hit alone

### Result R6. Meso-Scale Robustness

For UC1 under a denser noisy graph:

- `causal_reconstruction_score = 1.0` (same as micro)
- `retry_success_rate = 1.0` (same as micro)
- rendered token count: `282` (same as micro)

Interpretation:

- the explanatory signal survives distractor branches in the meso graph

## Primary Metrics

The paper should use a small set of defensible metrics:

- task success rate
- expected focus-node hit rate
- explanation roundtrip fidelity
- diagnostic rehydration-point hit rate
- causal reconstruction score
- rendered token count
- selected node count
- selected relationship count
- runtime portability score:
  - same task succeeds across multiple runtimes
- async trigger success rate
- end-to-end latency:
  - optional if we add timing instrumentation

## Baselines

We need simple baselines, not a benchmark zoo.

Recommended baselines:

1. Root-only baseline
   - root summary and root detail only
2. Flat detail baseline
   - all available details without graph relationships
3. Graph-without-detail baseline
   - nodes and relations only
4. Full kernel baseline
   - current node-centric rehydration

5. Structural-edge baseline
   - typed edges with ordering only
6. Explanatory-edge baseline
   - full kernel with typed relation explanations

## Abstract

Agent runtimes often depend on ad hoc prompt assembly or product-specific
context APIs, making context management difficult to reuse across systems. We
present `rehydration-kernel`, a graph-native context engine that reconstructs
bounded operational context from projection state and serves it through
generic query and async contracts. The kernel models context as nodes with
optional detail plus typed relationship explanations that preserve why
downstream nodes exist, including rationale, motivation, method, and
decision linkage. We evaluate the system with container-backed end-to-end
workflows across pull-driven and event-driven runtimes and four explanatory
use cases: failure diagnosis with rehydration-point recovery, reconstruction
of why a task was implemented in a particular way, interrupted handoff with
resumable execution, and constraint-preserving retrieval under token pressure.
Across these use cases, full explanatory context achieves `1.0` explanation
roundtrip fidelity and `1.0` causal reconstruction in the full-detail setting,
while retaining `1.0` causal reconstruction under a `192`-token budget in the
constraint-preserving case. Removing node detail preserves explanation fidelity
but reduces causal reconstruction to `0.857`, while replacing explanatory
relations with structural edges reduces causal reconstruction to `0.143`,
eliminates rehydration-point recovery, and fails to preserve the dominant
reason under budget pressure. These results indicate that explanatory
relationships carry the dominant signal for restartable and auditable agent
workflows, while node detail primarily improves completeness.

## Results Summary

The strongest result in the current artifact is not just that the kernel can
round-trip relationship explanations, but that those explanations dominate
task-relevant context quality.

Across all four explanatory use cases, `full_explanatory_with_detail` reaches
perfect `explanation_roundtrip_fidelity = 1.0` and
`causal_reconstruction_score = 1.0`. When detail is removed but explanatory
relations remain, causal reconstruction drops only to `0.857`. When
explanatory relations are replaced by structural edges while detail is kept,
causal reconstruction collapses to `0.143`. The net effect is an absolute
drop of `0.857` from losing explanation, versus `0.143` from losing detail.

This pattern also appears in recovery behavior. In UC1 and UC3, both
explanatory variants recover the correct rehydration or continuation point,
while structural-only and detail-only variants lose recovery entirely. The
closed-loop retry signal is even sharper: explanatory variants reach
`retry_success_rate = 1.0` while structural-only and detail-only stay at
`0.0`. That means the decisive recovery signal is carried by relationship
explanation fields such as `decision_id` and `caused_by_node_id`, not by
node detail alone.

UC4 provides the strongest bounded-retrieval result. The full explanatory
context rendered at `192` tokens still reaches `1.0` causal reconstruction
and preserves the dominant reason, while the structural-only variant at `96`
tokens drops to `0.125`.

The token results reinforce the same conclusion. Structural-only context is
shorter, but the reduction comes with a severe quality collapse: UC1 falls
from `282` to `200` rendered tokens, UC2 from `285` to `203`, and UC3 from
`575` to `374`, while causal reconstruction drops from `1.0` to `0.143` in
all cases. Removing detail gives a smaller token reduction alongside a much
smaller quality drop. The generated report and figures are written to:

- `artifacts/paper-use-cases/results.md`
- `artifacts/paper-use-cases/results.csv`
- `artifacts/paper-use-cases/results-figures.md`

These artifacts are sufficient to support the current paper claim: preserving
typed explanatory relationships is more important than preserving extended
node detail when the task is to diagnose, justify, or restart agent work.

## Conclusion

The current repository now supports a concrete and defensible systems claim:
context rehydration can be isolated into a reusable kernel, and explanatory
relations materially improve the usefulness of that context for agent
diagnosis and recovery. The experimental evidence spans four use cases with
multiple ablation variants. Across all use cases, explanatory relations
preserve the causal trace needed to explain why a task happened, identify
which relationships were suspect, select a correct rehydration point, and
preserve the dominant reason under token pressure. Detail remains useful, but
it behaves as a completeness amplifier rather than the primary carrier of
intent.

That gives the paper a sharper contribution than generic GraphRAG positioning.
The kernel is not only retrieving nearby nodes; it is preserving an
application-supplied explanation of transition between nodes and making that
explanation available through event, storage, query, and rendered-context
boundaries. The next step for a stronger submission is not to redesign the
model, but to widen the evaluation: larger graphs, stronger baselines,
unified cross-runtime metrics, and experiments that measure whether the
selected rehydration point leads to a corrected retry at scale.

## Draft Paper Outline

### 1. Introduction

- context management is a hidden systems problem in agent stacks
- prompt assembly is often runtime-specific and non-reusable
- graph-native rehydration offers a reusable alternative

### 2. Problem Statement

- agents need bounded, actionable context
- raw graph storage is not sufficient
- context retrieval should stay decoupled from runtime execution

### 3. System Design

- projection inputs over NATS
- graph state in Neo4j
- node detail in Valkey
- query serving over gRPC
- runtime boundary outside the kernel

### 4. Node-Centric Context Model

- root node
- neighbor nodes
- relationships
- extended node detail
- rendered bounded context

### 5. Experimental Setup

- Scenario A: minimal generic graph
- Scenario B: rich operational graph
- runtimes:
  - in-memory recording runtime
  - HTTP runtime adapter
- workflows:
  - pull-driven
  - event-driven

### 6. Results

- cross-runtime success
- async trigger success
- failure-diagnosis success
- why-implementation reconstruction success
- token-budget sensitivity
- subtree zoom behavior
- ablation outcomes

### 7. Limitations

- relation semantics are application-provided and not domain-validated beyond
  the typed explanation shape
- current generic gRPC path does not fully exploit focused-node rendering
- no auth or tenant isolation claims
- no long-lived session claims

### 8. Explanatory Relations

- typed relationship qualifiers
- decision-producing edges
- evidence-bearing edges
- narrative or causal ordering
- evaluation for explanation fidelity and diagnosis quality

### 9. Conclusion

- context rehydration can be a reusable kernel concern
- graph-native bounded retrieval is practical for agent systems
- diagnostic and recovery evaluation is the next step

## Explanatory Relations For The Stronger Paper

The stronger version of the paper is no longer about adding the explanatory
model. That part now exists in the kernel.

The stronger version is about proving what the explanatory model enables:

- diagnosis of wrong task implementations
- discovery of rehydration points
- reconstruction of why a task was implemented in a particular way
- measurable gains over structural-only relations

### Why This Matters

Today the kernel can answer:

- what nodes are near the root
- what detail belongs to each node
- what relationships exist

The stronger context model should answer:

- what produced this downstream node
- what decision justified it
- what evidence or rationale supports the transition
- what order the agent should reason through the neighborhood

That is the real bridge from graph retrieval to agent context management.

## Risks To Validity

- current agent tasks are still small and synthetic
- current success criteria are artifact-oriented, not benchmark-grade task
  scores
- runtime diversity is still limited to two implementations
- focus behavior still needs broader measurement across more agent entrypoints

These should be stated explicitly in the paper.

## Immediate Repo Work Before Submission

### Required For A Credible v1 Submission

- add cross-runtime metrics to the same paper artifact used for UC1-UC4
- capture end-to-end latency in the paper summary
- freeze the current artifacts into a release-ready paper appendix

Note: a denser meso graph (UC1) and detail-only baseline are now included in
the artifact.

### Required For The Stronger Explanatory Submission

- add root-only and flat-detail baselines alongside structural-only edges
- widen UC1-UC4 beyond synthetic local graphs
- add one longer causal chain with multiple competing decisions
- expand meso-scale variants to UC2-UC4

## Reproducibility Plan

Paper artifact commands should start from the existing scripts:

```bash
bash scripts/ci/integration-agentic-context.sh
bash scripts/ci/integration-agentic-event-context.sh
bash scripts/ci/integration-paper-use-cases.sh
```

Then add a paper harness that:

- runs both scenarios
- runs all ablations
- records success or failure
- records token counts
- records selected graph sizes
- emits a CSV or JSON summary

The current repo now has the first paper harness step for the explanatory
relation use cases:

- `scripts/ci/integration-paper-use-cases.sh`
- writes per-use-case metrics into `artifacts/paper-use-cases/cases/`
- writes an aggregated JSON summary into
  `artifacts/paper-use-cases/summary.json`
- writes a Markdown report into `artifacts/paper-use-cases/results.md`
- writes a CSV export into `artifacts/paper-use-cases/results.csv`
- writes Mermaid figures into `artifacts/paper-use-cases/results-figures.md`

## Recommendation

Write the paper in two layers:

- Paper v1:
  - generic node-centric context rehydration for agents
  - fully supported by the current repo with modest experiment harness work
- Paper v2:
  - explanatory graph relations improve diagnosis and task recovery
  - requires ablation results before the claim is strong enough

That sequencing keeps the paper technically honest while still aligning with
the direction you want: relationships that explain the decision that produced
the next node.

For the paper narrative, lead with the four use cases:

- diagnose a wrong implementation and recover from the correct rehydration
  point
- explain why a task was implemented in a particular way
- recover from an interrupted handoff and resume execution
- preserve the dominant constraint reason under token pressure

Those four stories are concrete, easy to evaluate, and tightly aligned with the
explanatory relation model.
