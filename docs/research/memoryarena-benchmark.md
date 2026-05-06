# MemoryArena Benchmark Adapter

Date: 2026-05-06
Status: feasibility adapter v1, stage-aware runner v1, and paper-aligned local scorecard v1 available

## Positioning

MemoryArena is the primary external benchmark candidate for the agentic-memory
track. It is a better product fit than pure chat QA because each task contains
multiple interdependent subtasks where later actions should use memory acquired
from earlier actions and feedback.

Sources:

- Project: `https://memoryarena.github.io/`
- Dataset: `https://huggingface.co/datasets/ZexueHe/memoryarena`
- Paper: `https://arxiv.org/abs/2602.16313`

LongMemEval remains the secondary conversational-memory regression. MemoryArena
is the first benchmark track intended to validate replayable, temporal,
multidimensional process memory.

## Official Source Check

Checked on 2026-05-06:

| Source | Observed status | Impact |
| --- | --- | --- |
| Project site | Describes MemoryArena as a multi-session Memory-Agent-Environment benchmark and links paper, data, and code. | Confirms this benchmark matches the kernel's agentic-memory direction. |
| Project `Code` link | Points to `https://github.com/`, not to a concrete repository. | There is no official evaluator repository available from the site in this cut. |
| Hugging Face dataset | Publishes five configs: `bundled_shopping`, `progressive_search`, `group_travel_planner`, `formal_reasoning_math`, and `formal_reasoning_phys`, each with `test` split. | The adapter should treat HF as the source of benchmark records. |
| Hugging Face files | Contains README plus JSONL files under the five config directories. | No official scoring script is published in the dataset repo. |
| Paper | Frames MemoryArena as multi-session Memory-Agent-Environment loops where agents learn from earlier feedback and reuse memory in later subtasks. | Our scorecard must remain clearly labelled as a local baseline until official code is published. |

Conclusion: official evaluator integration is currently blocked by missing
published evaluator code. The next implementable step is a paper-aligned
agentic reader/evaluator over KMP artifacts, while keeping the hook open to
swap in the official evaluator if the authors publish it.

## Adapter

The first adapter lives in `rehydration-testkit`:

```bash
cargo run -p rehydration-testkit --bin memoryarena_kmp_adapter --locked -- \
  --input /path/to/memoryarena/progressive_search/data.jsonl \
  --output artifacts/memoryarena-kmp/progressive-smoke \
  --task-type progressive_search \
  --limit 10
```

Supported input shape:

- JSONL, one MemoryArena task per line;
- `id`;
- `questions[]`;
- `answers[]`;
- optional `backgrounds` as string or list;
- optional `category`;
- optional `paper_name`;
- extra fields are preserved in parsing but not mapped into core KMP semantics
  yet.

The adapter is intentionally stage-aware. It does not ingest all answers before
all asks. For each task it emits:

```text
initial ingest        -> global background, when present
pre_subtask ingest    -> current question and subtask background
ask                   -> current subtask query
post_subtask ingest   -> answer feedback after the subtask is completed
```

This keeps MemoryArena aligned with agentic memory: an answer only becomes
memory for later subtasks after the environment has produced that feedback.

Dimension declarations are append-safe:

- the initial ingest declares the task and process dimensions;
- each episode dimension is declared only once;
- later appends can use `memory.dimensions: []` through the gRPC/MCP boundary
  once prior dimensions exist;
- sequential `follows` links use `class="procedural"`, because `temporal` is a
  traversal concept, not a valid relation semantic class.

Idempotency keys include the run scope so two benchmark runs with the same task
id do not collide when their `about` namespace changes.

## Runner

The live runner replays `events.jsonl` in order:

```bash
cargo run -p rehydration-testkit --bin memoryarena_kmp_runner --locked -- \
  --artifacts artifacts/memoryarena-kmp/progressive-smoke \
  --output artifacts/memoryarena-kmp/progressive-smoke-run \
  --endpoint http://rehydration-kernel.underpassai.com \
  --force
```

Generated runner artifacts:

| File | Purpose |
| --- | --- |
| `event_results.jsonl` | Per-event KMP success, elapsed time, and errors. |
| `results.jsonl` | Per-ask answer, proof, observed refs, missing refs, and known-at diagnostics. |
| `hypotheses.jsonl` | Compact answer stream for future evaluator integration. |
| `summary.json` | Aggregate event, known-at, leak, lexical, and evidence-ref counts. |

The runner treats failed KMP events as a non-zero run. It still writes the
diagnostic artifacts before returning the failure so ingestion, projection,
retrieval, or proof gaps remain inspectable.

## Scorecard

The deterministic scorecard consumes adapter and runner artifacts:

```bash
cargo run -p rehydration-testkit --bin memoryarena_kmp_scorecard --locked -- \
  --artifacts artifacts/memoryarena-kmp/progressive-smoke \
  --run artifacts/memoryarena-kmp/progressive-smoke-run \
  --output artifacts/memoryarena-kmp/progressive-smoke-scorecard \
  --force
```

Generated scorecard artifacts:

| File | Purpose |
| --- | --- |
| `subtask_results.jsonl` | Per-subtask hard correctness, domain-specific expected-answer parsing, optional soft score, known-at diagnostics, and ref recall. |
| `task_results.jsonl` | Per-task SR decision, PS, final-subtask score, extracted answers, known-at diagnostics, and ref recall. |
| `hypotheses.jsonl` | Compact chosen answer stream for evaluator or dashboard ingestion. |
| `score_summary.json` | Aggregate SR, PS, micro-PS, SR@depth, task-type summaries, candidate hit, known-at, leak, and runner metrics. |

The current scorecard is `memoryarena-kmp-scorecard-paper-aligned-v1`. It is a
paper-aligned local evaluator, not the official MemoryArena evaluator. It emits
the paper's core aggregate shape:

- `SR`: task success rate;
- `PS`: mean task progress score, computed as the fraction of hard-correct
  subtasks per task and then averaged across tasks;
- `micro_process_score`: hard-correct subtasks over all subtasks;
- `sr_at_depth`: subtask-depth success rate, aligned with the paper's
  subtask-depth decay analysis;
- `soft_process_score`: currently only a local proxy for domains with a partial
  scorer.

The evaluator keys subtasks by `(task_type, task_id, subtask_index)`, not by
`task_id` alone. This is required because MemoryArena task ids repeat across
dataset configs.

Task success follows the paper's domain distinction:

- `progressive_search`, `formal_reasoning_math`, and
  `formal_reasoning_phys`: final subtask correctness determines task success;
- `bundled_shopping` and `group_travel_planner`: all subtasks must be
  hard-correct because the final bundle or group plan must satisfy the whole
  accumulated task state.

Domain scoring:

- string answers use labelled `Exact Answer:` extraction, fallback normalized
  matching, and explicit alias handling such as `also written as`;
- shopping answers use `target_asin` as the hard success key and report
  attribute text coverage as a diagnostic soft score;
- group-travel answers use expected plan-slot text coverage as a local soft
  proxy. This is intentionally labelled as a proxy because the paper's true
  `sPS` is constraint-satisfaction based and the official environment evaluator
  has not been published.

See [memoryarena-paper-aligned-evaluator.md](memoryarena-paper-aligned-evaluator.md)
for the evaluator contract and extraction plan for a standalone public repo.

## Generated Artifacts

| File | Purpose |
| --- | --- |
| `events.jsonl` | Ordered mixed ingest/ask stream for a future stage-aware runner. |
| `ingest.jsonl` | KMP `kernel_ingest` events only. |
| `ask.jsonl` | KMP `kernel_ask` events only, with required ingest count and event index. |
| `expected.jsonl` | Expected answer and known-at-time refs for each subtask. |
| `replay.jsonl` | Timeline, known-at-time snapshots, and final path refs per task. |
| `summary.json` | Aggregate counts for tasks, subtasks, events, and backgrounds. |
| `manifest.json` | Run metadata and artifact paths. |

## Memory Mapping

For each task:

- `about`: `memoryarena:task_type:<task_type>:task:<id>`, or run-scoped when
  `--run-id` is provided;
- `benchmark_task` dimension scopes the whole task;
- `agentic_process` dimension scopes the ordered process;
- `agentic_episode` dimension scopes each subtask;
- `group_travel_planner` maps `base_person` into the initial global background
  because the paper initializes travel planning from a finalized base traveler
  state;
- background entries are available before the relevant subtask;
- question entries are written before the corresponding ask;
- answer-feedback entries are written after the corresponding ask;
- answer-feedback entries relate to their question with `rel="answers"`;
- later questions relate to the previous answer-feedback with `rel="follows"`.

The mapping is benchmark-neutral at the core level. It does not encode
MemoryArena scoring rules into KMP.

## Current Scope

Implemented:

- parser for JSON array or JSONL MemoryArena records;
- generic shape validation;
- KMP staged artifact generation;
- replay artifact generation;
- run-id isolation;
- live stage-aware runner against a deployed kernel;
- known-at correctness and future-answer leak diagnostics;
- paper-aligned local scorecard over runner artifacts;
- multi-config-safe runner and scorecard keys;
- incremental ingest through gRPC/MCP with empty dimension declarations after
  dimensions already exist;
- fixture tests and adapter smoke.

Not implemented yet:

- MemoryArena official evaluator integration, blocked until official code is
  published or discoverable;
- agentic retrieval loop that lets an LLM choose multiple KMP moves before
  answering;
- benchmark dashboard or graph visualization.

## Verification

Fixture smoke:

```bash
cargo run -p rehydration-testkit --bin memoryarena_kmp_adapter --locked -- \
  --input crates/rehydration-testkit/tests/fixtures/memoryarena_minimal.jsonl \
  --output /tmp/memoryarena-kmp-adapter-smoke \
  --task-type progressive_search \
  --force
```

Expected summary for the fixture:

```text
dataset_items: 1
prepared_tasks: 1
subtasks: 2
ingest_events: 5
ask_events: 2
replay_events: 7
background_entries: 1
```

The second subtask's known-at-time snapshot includes the global background, the
first question, the first answer feedback, and the second question. It does not
include the second answer before the second ask.

Live runner smoke against the public kernel endpoint:

```bash
cargo run -p rehydration-testkit --bin memoryarena_kmp_adapter --locked -- \
  --input crates/rehydration-testkit/tests/fixtures/memoryarena_minimal.jsonl \
  --output /tmp/memoryarena-kmp-adapter-smoke \
  --task-type progressive_search \
  --run-id codex-roadmap-20260506d \
  --force

cargo run -p rehydration-testkit --bin memoryarena_kmp_runner --locked -- \
  --artifacts /tmp/memoryarena-kmp-adapter-smoke \
  --output /tmp/memoryarena-kmp-run-smoke \
  --endpoint http://rehydration-kernel.underpassai.com \
  --force
```

Observed summary:

```text
total_events: 7
successful_events: 7
failed_events: 0
known_at_clean_asks: 2
future_answer_leaks: 0
current_question_observed: 2
unexpected_ref_asks: 0
missing_allowed_ref_asks: 0
```

The first ask correctly returns `UNKNOWN` because the answer feedback is not yet
known. The second ask retrieves the first answer feedback and proves it through
the staged memory path without leaking the second answer.

## Real Progressive Search Slice

Run date: 2026-05-06

Dataset source:

- `ZexueHe/memoryarena`
- config: `progressive_search`
- split: `test`
- Dataset Viewer total rows: 221
- slice: `offset=0`, `length=3`

Dataset export:

```bash
curl -s -o /tmp/memoryarena-progressive-rows-3.json \
  'https://datasets-server.huggingface.co/rows?dataset=ZexueHe/memoryarena&config=progressive_search&split=test&offset=0&length=3'

jq -c '.rows[].row' /tmp/memoryarena-progressive-rows-3.json \
  > /tmp/memoryarena-progressive-real-3.jsonl
```

Slice shape:

```text
task 0: 9 questions / 9 answers
task 1: 12 questions / 12 answers
task 2: 6 questions / 6 answers
```

Adapter command:

```bash
cargo run -p rehydration-testkit --bin memoryarena_kmp_adapter --locked -- \
  --input /tmp/memoryarena-progressive-real-3.jsonl \
  --output /tmp/memoryarena-progressive-real-3-artifacts \
  --task-type progressive_search \
  --run-id memoryarena-real-20260506-s3 \
  --force
```

Adapter summary:

```text
dataset_items: 3
prepared_tasks: 3
subtasks: 27
ingest_events: 54
ask_events: 27
replay_events: 81
background_entries: 0
```

Runner command:

```bash
cargo run -p rehydration-testkit --bin memoryarena_kmp_runner --locked -- \
  --artifacts /tmp/memoryarena-progressive-real-3-artifacts \
  --output /tmp/memoryarena-progressive-real-3-run \
  --endpoint http://rehydration-kernel.underpassai.com \
  --force
```

Runner summary:

```text
total_events: 81
successful_events: 81
failed_events: 0
known_at_clean_asks: 27
future_answer_leaks: 0
current_question_observed: 27
unexpected_ref_asks: 0
missing_allowed_ref_asks: 0
elapsed_ms: 106581
```

Interpretation:

- The deployed kernel accepted a real staged MemoryArena slice end to end.
- Every ask observed the current question ref.
- Every ask respected known-at-time boundaries.
- No ask leaked its current answer feedback before that feedback was ingested.
- Every ask returned all refs that were expected to be available at that point
  in the staged process.
- `lexical_answer_hits` is not meaningful for this staged run because
  `expected.answer` represents environment feedback that intentionally becomes
  memory only after the ask. Task-success scoring belongs in the next evaluator
  layer.

Scorecard command:

```bash
cargo run -p rehydration-testkit --bin memoryarena_kmp_scorecard --locked -- \
  --artifacts /tmp/memoryarena-progressive-real-3-artifacts \
  --run /tmp/memoryarena-progressive-real-3-run \
  --output /tmp/memoryarena-progressive-real-3-scorecard \
  --force
```

Scorecard summary:

```text
scorecard: memoryarena-kmp-scorecard-paper-aligned-v1
schema_version: memoryarena-score-summary-v1
tasks: 3
subtasks: 27
ask_count: 27
task_successes: 3
task_success_rate: 1.0
passed_subtasks: 24
process_score: 0.8796296296296297
micro_process_score: 0.8888888888888888
candidate_answer_hits: 3
candidate_answer_hit_rate: 1.0
known_at_clean_asks: 27
full_ref_recall_asks: 27
current_question_observed_asks: 27
future_answer_leaks: 0
unexpected_ref_asks: 0
missing_allowed_ref_asks: 0
runner_total_events: 81
runner_successful_events: 81
runner_failed_events: 0
runner_elapsed_ms: 106581
```

Interpretation:

- The paper-aligned local scorecard recovered the final exact answer for all
  three tasks in this first `progressive_search` slice, so paper-style `SR`
  is `1.0` for this slice.
- `PS` is lower than `SR` because early progressive subtasks can be correct
  process memory without the current answer text being emitted by
  `kernel_ask.answer`. This is useful signal for separating memory substrate
  behavior from reader/agent answer formatting.
- One task required alias-aware matching: the expected answer used `Daniel Delos
  Santos`, while the retrieved candidate used `John Daniel delos Santos` with an
  explicit alias.
- This is a local reader baseline over kernel evidence, not an official
  MemoryArena score.

Observed consumer gap:

- The deterministic `answer` field can grow as accumulated prior feedback grows.
  This is acceptable for proof/evidence validation, but not sufficient as the
  final agentic benchmark answer. The next layer should score task success from
  an agent/reader that consumes the kernel refs and can choose traversal moves
  before producing the final answer.

## Realistic Two-Per-Domain Slice

Run date: 2026-05-06

Dataset source:

- `ZexueHe/memoryarena`
- configs: all five MemoryArena configs
- split: `test`
- slice: first two valid tasks per config

Slice shape:

| Domain | Tasks | Subtasks |
| --- | ---: | ---: |
| `bundled_shopping` | 2 | 12 |
| `progressive_search` | 2 | 21 |
| `group_travel_planner` | 2 | 15 |
| `formal_reasoning_math` | 2 | 10 |
| `formal_reasoning_phys` | 2 | 15 |
| Total | 10 | 73 |

Live runner result against `http://rehydration-kernel.underpassai.com`:

```text
events: 221/221 successful
asks: 73/73 known-at-clean
full_ref_recall_asks: 73/73
future_answer_leaks: 0
unexpected_ref_asks: 0
missing_allowed_ref_asks: 0
```

Combined scorecard result:

```text
tasks: 10
subtasks: 73
task_successes: 3
task_success_rate: 0.3000
passed_subtasks: 29
process_score: 0.3622
micro_process_score: 0.3973
known_at_clean_asks: 73
full_ref_recall_asks: 73
current_question_observed_asks: 73
future_answer_leaks: 0
unexpected_ref_asks: 0
missing_allowed_ref_asks: 0
```

By domain:

| Domain | Task SR | Passed subtasks | PS | Micro PS | Soft PS |
| --- | ---: | ---: | ---: | ---: | ---: |
| `bundled_shopping` | 0.0000 | 0/12 | 0.0000 | 0.0000 | 0.0167 |
| `progressive_search` | 1.0000 | 19/21 | 0.9028 | 0.9048 | n/a |
| `group_travel_planner` | 0.0000 | 0/15 | 0.0000 | 0.0000 | 0.5837 |
| `formal_reasoning_math` | 0.0000 | 2/10 | 0.2000 | 0.2000 | n/a |
| `formal_reasoning_phys` | 0.5000 | 8/15 | 0.7083 | 0.5333 | n/a |

Interpretation:

- The kernel substrate passed the memory part of the slice: no temporal leaks,
  no missing allowed refs, no unexpected refs, and every ask observed the
  current question.
- `progressive_search` is the cleanest current fit because later answers can
  often be recovered directly from prior feedback.
- `bundled_shopping` and `group_travel_planner` need an agent/environment or
  domain reader. The kernel retrieves the staged evidence, but `kernel_ask`
  currently replays prior feedback instead of choosing the current product or
  composing the current itinerary.
- The formal domains need a specialized exact-answer reader over recovered
  evidence; otherwise long mathematical/physics context is surfaced correctly
  but not reduced to the requested formula or statement.
- This result should not be reported as an official MemoryArena score. It is a
  kernel-backed local scorecard that separates memory recall from task
  reasoning and answer construction.

## Next Cut

1. Add benchmark reader/plugin layers for shopping, travel, and formal exact
   answer extraction, outside the kernel core.
2. Add an agentic retrieval loop that can choose `ask`, `near`, `trace`,
   `inspect`, and temporal moves before answering.
3. Persist projection/traversal/proof metrics needed to classify benchmark
   failures without reading raw logs.
4. Extract the paper-aligned evaluator into a standalone public repository once
   the metric contract and fixture suite are stable.
