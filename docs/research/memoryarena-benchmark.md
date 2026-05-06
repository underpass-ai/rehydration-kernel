# MemoryArena Benchmark Adapter

Date: 2026-05-06
Status: feasibility adapter v1 and stage-aware runner v1 available

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
- incremental ingest through gRPC/MCP with empty dimension declarations after
  dimensions already exist;
- fixture tests and adapter smoke.

Not implemented yet:

- MemoryArena official evaluator integration;
- task-success scoring;
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

Observed consumer gap:

- The deterministic `answer` field can grow as accumulated prior feedback grows.
  This is acceptable for proof/evidence validation, but not sufficient as the
  final agentic benchmark answer. The next layer should score task success from
  an agent/reader that consumes the kernel refs and can choose traversal moves
  before producing the final answer.

## Next Cut

1. Add task-success scoring and official evaluator integration.
2. Add an agentic retrieval loop that can choose `ask`, `near`, `trace`,
   `inspect`, and temporal moves before answering.
3. Persist projection/traversal/proof metrics needed to classify benchmark
   failures without reading raw logs.
