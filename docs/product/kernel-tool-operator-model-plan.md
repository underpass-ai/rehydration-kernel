# Kernel Tool Operator Model Plan

Status: P1 prepared for the next execution slice.

Date: 2026-05-08.

## Goal

Build a small specialist model that operates the Kernel Memory Protocol tools
efficiently. The model is not the kernel, not the reader, and not the memory
reasoner. It is a tool operator that decides which bounded KMP/MCP move to call
next, with which arguments, and when to stop.

The first useful outcome is not model training. The first useful outcome is a
clean trajectory dataset and evaluator that make tool-use quality measurable.

## Boundary

The operator model may:

- choose one KMP/MCP tool call at a time;
- choose scoped, bounded arguments;
- decide whether to continue, inspect, trace, move temporally, or stop;
- propose `kernel_write_memory` relations only when the relation cites evidence
  observed through previous kernel reads.

The operator model must not:

- bypass the public KMP/MCP/gRPC contract;
- become a hidden memory API;
- invent refs, evidence, relations, dimensions, or about scopes;
- request unbounded histories by default;
- own answer semantics for benchmark-specific tasks;
- become a dependency of kernel core.

Kernel responsibilities remain deterministic:

- traversal;
- proof;
- validation;
- fail-fast errors;
- audit;
- persistence;
- projection.

## Tools In Scope

Read/navigation tools:

- `kernel_ask`
- `kernel_near`
- `kernel_trace`
- `kernel_inspect`
- `kernel_goto`
- `kernel_rewind`
- `kernel_forward`

Write tool:

- `kernel_write_memory`

Write mode is a separate policy mode. The first operator slice should focus on
read/navigation trajectories, then add writer relation trajectories after the
read loop is measurable.

## Why This Is P1

The current MemoryArena and MCP runs show that the kernel can expose useful
memory, but generalist LLM tool use is inconsistent:

- it may overuse broad traversal;
- it may not choose the cheapest next move;
- it may stop late or stop with the wrong candidate;
- it may need more context than a kernel-native operator should need;
- its reasoning is hard to compare unless tool trajectories are normalized.

A small model can be viable because the task surface is narrow:

- state is structured;
- available actions are finite;
- valid arguments are constrained;
- success is directly measurable from refs, scopes, leaks, and stop decisions.

## Required Contract Before Training

Do not start model training until these are true:

- MCP tool schemas are stable enough to serialize into the training prompt.
- `near`, `trace`, `goto`, `rewind`, and `forward` expose bounded behavior and
  pagination consistently.
- Tool errors are fail-fast and explicit enough to be useful training labels.
- Raw logs are redacted: no API keys, credentials, private raw memory, or
  unbounded prompt dumps.
- Successful and failed trajectories can be separated deterministically.

Training on broken traces without labels is prohibited. Failed trajectories are
useful as hard negatives only after the failure reason is classified.

## Trajectory Schema

Each training/evaluation item should be one decision step, not one whole
benchmark task.

Minimal shape:

```json
{
  "schema_version": "kernel-operator-trajectory-v1",
  "run_id": "memoryarena-smart-writer-paged-50tasks-20260508-0009",
  "task_family": "memoryarena.progressive_search",
  "mode": "read",
  "about": "memoryarena:run:...",
  "step_index": 4,
  "goal": "Find the next bounded evidence move for the current question.",
  "visible_state": {
    "current_ref": "memoryarena:...:subtask:9:answer",
    "known_refs": ["..."],
    "last_tool": "kernel_near",
    "last_observed_refs": ["..."],
    "remaining_budget": {
      "tool_calls": 4,
      "context_chars": 12000
    }
  },
  "allowed_tools": [
    "kernel_near",
    "kernel_trace",
    "kernel_inspect",
    "kernel_goto",
    "kernel_rewind",
    "kernel_forward",
    "kernel_ask"
  ],
  "target_action": {
    "type": "tool_call",
    "tool": "kernel_inspect",
    "arguments": {
      "about": "memoryarena:run:...",
      "ref": "memoryarena:...:subtask:9:answer",
      "include": {
        "details": true,
        "relationships": true,
        "raw": false
      }
    }
  },
  "observed_outcome": {
    "success": true,
    "observed_refs": ["..."],
    "elapsed_ms": 46
  },
  "quality": {
    "known_at_clean": true,
    "future_answer_leak": false,
    "invalid_tool_call": false,
    "bounded": true,
    "stop_correct": null
  }
}
```

Stop action shape:

```json
{
  "target_action": {
    "type": "stop",
    "answer_policy": "evidence_or_unknown",
    "final_refs": ["..."],
    "reason": "sufficient_evidence"
  }
}
```

The dataset should preserve the raw tool result path separately for audit, but
the model-facing item should be compact and redacted.

## Initial Data Sources

Use already audited runs first:

- MemoryArena 100-task smart-writer run:
  `/tmp/memoryarena-smart-writer-paged-100tasks-20260508-1407-run`
- MemoryArena 100-task stderr/log digest:
  `/tmp/memoryarena-smart-writer-paged-100tasks-20260508-1407.stderr`
- MemoryArena 100-task scorecard:
  `/tmp/memoryarena-smart-writer-paged-100tasks-20260508-1407-scorecard`
- MemoryArena 100-task audit:
  `/tmp/memoryarena-smart-writer-paged-100tasks-20260508-1407-audit.json`
- MemoryArena 50-task smart-writer run:
  `/tmp/memoryarena-smart-writer-paged-50tasks-20260508-0009-run`
- MemoryArena 50-task stderr/log digest:
  `/tmp/memoryarena-smart-writer-paged-50tasks-20260508-0009.stderr`
- MemoryArena 50-task scorecard:
  `/tmp/memoryarena-smart-writer-paged-50tasks-20260508-0009-scorecard`
- MemoryArena prepared 221-task artifacts:
  `/tmp/memoryarena-progressive-221-cost-estimate-artifacts`
- Live MCP story demo docs:
  `docs/research/kernel-memory-story-demo-2026-05-05.md`
- Live mobile incident MCP demo docs:
  `docs/research/mobile-login-resolution-replay-demo-2026-05-05.md`

The 100-task MemoryArena run is the best first corpus because it includes real
MCP navigation, smart-writer read-before-write decisions, relation quality
metrics, known-at checks, bounded trace proofs, and three final reader failures
where evidence recall was complete. Those failures prove the operator must not
be evaluated only by final answer correctness.

## P1 Work Packages

### P1.1 Trajectory Exporter

Add a testkit exporter that converts existing runner logs and artifacts into
`kernel-operator-trajectory-v1`.

Proposed binary:

```text
kernel_operator_trajectory_export
```

Inputs:

- MemoryArena runner output directory;
- MemoryArena scorecard directory;
- optional stderr JSONL log with MCP read/write events;
- optional run id filter;
- optional task id filter.

Outputs:

- `trajectories.jsonl`;
- `summary.json`;
- `failures.jsonl`;
- `redaction_report.json`.

Exit criteria:

- exporter is deterministic;
- exporter fails fast on mixed runs;
- no secret-looking values are emitted;
- every target tool call has bounded arguments;
- failed and successful steps are labelled separately.

### P1.2 Operator Evaluator

Add an offline evaluator that scores a predicted tool action against the
recorded target action and final trajectory quality.

Metrics:

- tool selection accuracy;
- argument validity;
- scope correctness;
- ref correctness;
- bounded/paginated request correctness;
- invalid tool call rate;
- future leak rate;
- known-at clean rate;
- stop accuracy;
- excess tool calls;
- elapsed latency by tool;
- answer-independent trajectory success.

Exit criteria:

- a generalist LLM baseline and a deterministic baseline can be compared;
- evaluation does not require access to expected benchmark answers;
- MemoryArena task `11` is classified as a reader/candidate failure, not an
  operator failure, when navigation/proof is clean.

### P1.3 Deterministic Baseline

Implement a simple rule baseline before training:

- inspect the current anchor;
- near around the latest known prior ref;
- trace only when there is a target ref and a bounded path need;
- stop when proof refs cover the current ask and no future refs appear.

This baseline is intentionally modest. It gives the operator model something
concrete to beat.

### P1.4 Generalist LLM Baseline

Replay the same trajectory items with a generalist model that emits only one
of:

- a JSON tool call;
- a JSON stop action.

The prompt must include:

- compact tool schema;
- explicit bounds;
- current visible state;
- no benchmark gold answer;
- no raw hidden memory.

This baseline measures whether a small operator can reduce tool calls, context
usage, invalid calls, and latency while preserving ref quality.

### P1.5 First Small-Model Experiment

Only after P1.1-P1.4:

- choose a small function-calling or instruction model;
- train or fine-tune on exported trajectories;
- evaluate on held-out runs and task families;
- compare to deterministic and generalist baselines.

No model should be added to kernel core. It runs as a sidecar/client.

## Fast Training Path

The goal is fast iteration, not a large first model. The first operator model
should prove that a small specialist can choose bounded kernel tools better
than a generalist baseline on the same replayable trajectories.

### Hardware Assumption

Target local hardware:

- 4x RTX 3090;
- 24 GB VRAM per GPU;
- local fine-tuning with LoRA/QLoRA;
- offline replay evaluation before live-kernel validation.

Expected first useful training durations:

| Model size | Method | Expected iteration time |
| --- | --- | --- |
| 0.5B-1.5B | LoRA or full small-model fine-tune | 30 min - 3 h |
| 3B | LoRA | 2 - 6 h |
| 7B-8B | QLoRA | 6 - 18 h |
| 14B | QLoRA | 1 - 3 days |

The first slice should prefer 0.5B-1.5B or 3B. A 7B/8B model is useful only
after the dataset and evaluator are clean.

### Speed Principles

1. Train on decisions, not whole tasks.

Each item should be:

```text
visible_state -> next_tool_call_or_stop
```

Do not include full conversations, full graph dumps, or hidden benchmark
answers in the model-facing input.

2. Start with a small action space.

Initial read/navigation actions:

- `kernel_near`
- `kernel_inspect`
- `kernel_trace`
- `stop`

Hold back until later:

- `kernel_goto`
- `kernel_rewind`
- `kernel_forward`
- `kernel_write_memory`

This reduces ambiguity and lets the first model learn the highest-frequency
navigation moves before temporal movement and write policy are added.

3. Distill from multiple sources.

Generate candidate trajectories from:

- successful audited MemoryArena MCP runs;
- deterministic navigation baseline;
- generalist LLM baseline;
- classified hard negatives.

Only train on failed trajectories when the failure reason is labelled. Broken
logs without labels should be excluded.

4. Use curriculum stages.

Recommended stages:

- valid JSON action;
- valid tool name;
- bounded arguments;
- valid refs and scopes;
- correct next tool;
- correct stop decision;
- pagination and temporal movement;
- writer relation proposal with cited evidence.

Do not mix all stages on day one. If JSON validity or bounded arguments fail,
later training metrics are not meaningful.

5. Cache evaluation.

The evaluator should run offline from exported trajectories and cached tool
results. Live kernel calls are reserved for final validation, not every training
iteration.

6. Measure cheap metrics first.

Fast-loop metrics:

- JSON validity;
- tool selection accuracy;
- invalid ref rate;
- invalid scope rate;
- unbounded call rate;
- stop accuracy;
- excess tool calls;
- future leak rate;
- known-at clean rate.

Do not wait for an end-to-end benchmark score to decide whether a checkpoint is
usable.

### Five-Day Fast Track

Day 1:

- implement the trajectory exporter;
- export the MemoryArena 50-task run;
- validate redaction and mixed-run rejection.

Day 2:

- implement deterministic baseline;
- implement offline evaluator;
- produce baseline summary.

Day 3:

- train first 0.5B-1.5B or 3B LoRA checkpoint;
- evaluate offline against deterministic baseline.

Day 4:

- run a generalist LLM baseline on the same trajectory items;
- compare tool calls, invalid actions, boundedness, and stop decisions.

Day 5:

- run live-kernel validation on a small held-out slice;
- package dataset card, model card draft, and evaluation summary for Hugging
  Face publication.

### Hugging Face Publication Path

Publish only after redaction and eval are credible.

Recommended artifacts:

- dataset: `underpass-ai/kernel-operator-trajectories`;
- model: `underpass-ai/kernel-tool-operator-small`;
- optional Space: `underpass-ai/kernel-operator-demo`.

Correct positioning:

> A small specialist model trained to operate Underpass Kernel memory tools
> through bounded, auditable trajectories.

Avoid claims that it is a general QA model, a memory database, or a replacement
for the kernel.

## Tomorrow's First Slice

Recommended first execution slice:

1. Create `kernel-operator-trajectory-v1` Rust structs in `rehydration-testkit`.
2. Implement exporter support for MemoryArena smart-writer logs only.
3. Export the 50-task run into `/tmp/kernel-operator-trajectories-50`.
4. Add unit tests for redaction, mixed-run rejection, and bounded-action shape.
5. Document the exported summary in this file or `memoryarena-benchmark.md`.

Do not start model training tomorrow unless the exporter and evaluator are
already producing clean data. Bad operator data will be worse than no model.

## Open Questions

- Should write-mode trajectories be a separate dataset from read-mode
  trajectories? Initial answer: yes.
- Should the operator be allowed to call `kernel_ask`, or should `ask` be the
  reader's terminal move? Initial answer: allow it, but score overuse.
- Should the model learn relation writing directly? Initial answer: only after
  read/navigation is stable, and only with proof-cited relations.
- Should the first operator be trained or just prompted? Initial answer: export
  trajectories and run baselines before deciding.

## Success Criteria For P1

P1 is successful when we can say:

- kernel tool-use trajectories are exported reproducibly;
- a baseline can be scored without benchmark gold answers;
- the operator policy is bounded and auditable;
- tool-call efficiency can be compared against a generalist LLM;
- failures are classified by stage: operator, kernel retrieval, reader, writer,
  or benchmark/domain reasoning.

The article angle then becomes stronger:

> The kernel does not only store temporal multidimensional memory. It emits
> auditable tool-use trajectories that can train and evaluate smaller specialist
> operators for memory navigation.
