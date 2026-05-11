# Kernel Tool Operator Model Plan

Status: P1.1 trajectory exporter, P1.2/P1.3 offline evaluator/deterministic
baseline, P1.4 generalist LLM baseline CLI, and P1.5 SFT training are
implemented. The row-split LoRA run remains only a smoke test. The preferred
result is now the V6 explicit holdout20 run: grouped task split with tasks
80-99 reserved for eval, synthetic model-facing refs, zero dropped non-visible
target refs, structural writer candidate details, and evaluation against
anonymized model trajectories. It reached 1.000 exact action accuracy, 1.000
primary-ref accuracy, and zero invalid or unbounded actions on 1,124 held-out
decisions.

Date: 2026-05-11.

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

Implemented binary:

```text
cargo run -p rehydration-testkit --bin kernel_operator_trajectory_export -- \
  --run <memoryarena-run-dir> \
  --output <output-dir> \
  [--include-writer-reads] \
  [--expected-run-id <id>] \
  [--force]
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

- [x] exporter is deterministic;
- [x] exporter fails fast on mixed runs;
- [x] no secret-looking values are emitted;
- [x] every target tool call has bounded arguments;
- [x] failed and successful steps are labelled separately.

Implemented outputs:

- `trajectories.jsonl`
- `summary.json`
- `failures.jsonl`
- `redaction_report.json`

The model-facing trajectory intentionally excludes benchmark gold answers,
`ask_answer`, raw `ask_content`, API keys, credentials, and raw prompt dumps.
Each item is a single decision step:

```text
visible_state -> target_action
```

The first exported action space is read/navigation:

- `kernel_near`
- `kernel_inspect`
- `kernel_trace`
- `stop`

`--include-writer-reads` additionally exports read-before-write tool calls from
the smart writer as `write_context_read` mode, but it does not yet train the
model to propose `kernel_write_memory` relations.

Real export checks:

| Source run | Mode | Trajectories | Tool calls | Stops | Failures | Redaction findings |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| MemoryArena 50-task smart-writer run | read | 1,426 | 1,057 | 369 | 0 | 0 |
| MemoryArena 50-task smart-writer run | read + writer reads | 2,802 | 2,433 | 369 | 0 | 0 |
| MemoryArena 100-task smart-writer run | read | 2,912 | 2,159 | 753 | 0 | 0 |
| MemoryArena 100-task smart-writer run | read + writer reads with candidates | 5,724 | 4,971 | 753 | 0 | 0 |

The first 100-task read-only export produced:

```text
kernel_near    753
kernel_inspect 753
kernel_trace   653
stop           753
```

This gives enough clean data to implement P1.2 without touching kernel core.

### P1.2 Operator Evaluator

Add an offline evaluator that scores a predicted tool action against the
recorded target action and final trajectory quality.

Implemented binary:

```text
cargo run -p rehydration-testkit --bin kernel_operator_policy_eval -- \
  --trajectories <trajectories.jsonl> \
  [--predictions <predictions.jsonl>] \
  [--baseline deterministic|oracle] \
  [--output <summary.json>]
```

Prediction rows may use either:

```json
{"step_id": "...", "action": {"type": "tool_call", "tool": "...", "arguments": {}}}
```

or:

```json
{"step_id": "...", "target_action": {"type": "stop", "reason": "..."}}
```

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

- [x] a generalist LLM baseline and a deterministic baseline can be compared;
- [x] evaluation does not require access to expected benchmark answers;
- [ ] MemoryArena task `11` is classified as a reader/candidate failure, not an
  operator failure, when navigation/proof is clean.

Current evaluator metrics:

- missing predictions;
- invalid predictions;
- unbounded tool calls;
- action type accuracy;
- tool accuracy over target tool calls;
- primary ref accuracy over target tool calls;
- scope accuracy over target tool calls;
- stop accuracy over target stop actions;
- exact action accuracy.

Real deterministic baseline checks:

| Source trajectories | Tool accuracy | Ref accuracy | Scope accuracy | Stop accuracy | Invalid | Unbounded |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| MemoryArena 50-task read trajectories | 1.000 | 1.000 | 1.000 | 1.000 | 0 | 0 |
| MemoryArena 100-task read trajectories | 1.000 | 1.000 | 1.000 | 1.000 | 0 | 0 |

The exact JSON action score is lower, about `0.517`, because the deterministic
baseline intentionally reconstructs equivalent bounded actions and stops rather
than copying auxiliary `goal` text or exact `final_refs` from the recorded
target. For model comparison, tool/ref/scope/stop validity is the primary P1
signal.

### P1.3 Deterministic Baseline

Implement a simple rule baseline before training:

- inspect the current anchor;
- near around the latest known prior ref;
- trace only when there is a target ref and a bounded path need;
- stop when proof refs cover the current ask and no future refs appear.

This baseline is intentionally modest. It gives the operator model something
concrete to beat.

Implemented inside `kernel_operator_policy_eval` as `--baseline deterministic`.
It is not a trained model. It exists to provide a stable floor for future
generalist and small-model comparisons.

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

Implemented binary:

```text
cargo run -p rehydration-testkit --bin kernel_operator_llm_baseline -- \
  --trajectories <trajectory-dir>/trajectories.jsonl \
  --output <output-dir> \
  --endpoint <openai-compatible-chat-completions-url> \
  --model <model> \
  [--provider openai|openai-new|anthropic] \
  [--api-key-env LLM_API_KEY] \
  [--limit n] \
  [--offset n] \
  [--max-refs n] \
  [--force]
```

Outputs:

- `predictions.jsonl`: clean evaluator input with only `step_id` and `action`;
- `llm_results.jsonl`: raw model response, token counts, latency, and parsed
  action for audit;
- `failures.jsonl`: LLM call failures or rejected actions;
- `summary.json`: selected trajectory count, prediction count, invalid count,
  boundedness count, token usage, latency, and action distribution.

Guardrails:

- no `target_action`, `observed_outcome`, benchmark gold answer, or hidden raw
  memory is included in the prompt;
- visible refs are capped with `--max-refs`;
- rejected actions do not enter `predictions.jsonl`;
- tool calls must be bounded;
- tool names must be present in the trajectory action space;
- tool refs and stop refs must already be visible in `current_ref`,
  `trace_target_ref`, `known_refs`, or `last_observed_refs`;
- `kernel_inspect.include.raw` must be `false`.

Evaluation command after a run:

```text
cargo run -p rehydration-testkit --bin kernel_operator_policy_eval -- \
  --trajectories <trajectory-dir>/trajectories.jsonl \
  --predictions <llm-baseline-dir>/predictions.jsonl \
  --output <llm-baseline-dir>/policy-eval.json \
  [--limit n] \
  [--offset n]
```

Real OpenAI baseline smoke:

| Model | Slice | Prompt | Predictions | Failures | Exact | Tool | Ref | Scope | Stop | Invalid | Unbounded |
| --- | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `gpt-4o-mini` | first 30 trajectories | initial | 23 | 7 | 0.300 | 0.696 | 0.696 | 0.696 | 0.857 | 0 | 0 |
| `gpt-4o-mini` | first 30 trajectories | explicit `about` rule | 30 | 0 | 0.533 | 1.000 | 1.000 | 1.000 | 1.000 | 0 | 0 |

The initial failure mode was useful: the generalist model used `current_ref` as
`kernel_near.arguments.about` instead of the top-level `about`. The prompt and
training data now state explicitly that `about` must equal the top-level scope.

### P1.5 First Small-Model Experiment

Only after P1.1-P1.4:

- choose a small function-calling or instruction model;
- train or fine-tune on exported trajectories;
- evaluate on held-out runs and task families;
- compare to deterministic and generalist baselines.

No model should be added to kernel core. It runs as a sidecar/client.

Implemented local training path:

```text
scripts/operator/prepare_operator_sft_dataset.py
scripts/operator/train_operator_sft_lora.py
scripts/operator/predict_operator_sft.py
scripts/operator/README.md
```

The first SFT dataset has been prepared from the 100-task trajectory export:

```text
python scripts/operator/prepare_operator_sft_dataset.py \
  --trajectories /tmp/kernel-operator-trajectories-100/trajectories.jsonl \
  --output /tmp/kernel-operator-sft-100 \
  --eval-ratio 0.1 \
  --seed 42 \
  --force
```

Result:

| Source | Selected | Train | Eval | Near | Inspect | Trace | Stop |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| MemoryArena 100-task read trajectories | 2,912 | 2,621 | 291 | 753 | 753 | 653 | 753 |

Leak check:

```text
rg -n 'target_action|observed_outcome|quality|gold|answer_session|has_answer' \
  /tmp/kernel-operator-sft-100/train.jsonl \
  /tmp/kernel-operator-sft-100/eval.jsonl
```

returned no matches. The target action exists only in the assistant message,
not in the model-facing user prompt.

Recommended first model:

```text
Qwen/Qwen2.5-0.5B-Instruct
```

It is small enough for fast iteration and good enough to validate whether the
task is learnable before spending time on 1.5B/3B/7B variants.

First local training command:

```text
python scripts/operator/train_operator_sft_lora.py \
  --train-jsonl /tmp/kernel-operator-sft-100/train.jsonl \
  --eval-jsonl /tmp/kernel-operator-sft-100/eval.jsonl \
  --model-id Qwen/Qwen2.5-0.5B-Instruct \
  --output-dir /tmp/kernel-operator-qwen05-lora \
  --epochs 3 \
  --batch-size 2 \
  --grad-accum 8 \
  --max-length 2048 \
  --bf16
```

Use `--fp16` instead of `--bf16` if the active GPU does not support bfloat16.

Current execution blocker in the Codex shell:

- `nvidia-smi` cannot communicate with the NVIDIA driver;
- Python training dependencies are not installed in this environment
  (`torch`, `transformers`, `trl`, `peft`);
- OpenAI accepted file upload, but fine-tuning is unavailable for the current
  organization (`training_not_available`).

These are environment issues, not dataset or code issues.

First local Kubernetes training run:

```text
kubectl apply -f k8s/kernel-operator-qwen05-lora-job.yaml
```

Training environment:

- model: `Qwen/Qwen2.5-0.5B-Instruct`;
- method: LoRA SFT;
- GPU: 1x RTX 3090 requested through `nvidia.com/gpu: 1`;
- train rows: 2,621;
- eval rows: 291;
- epochs: 3;
- wall time: 58 minutes.

Final training metrics:

| Metric | Value |
| --- | ---: |
| train_loss | 0.05115 |
| eval_loss | 0.008344 |
| eval_mean_token_accuracy | 0.9965 |
| eval_samples_per_second | 6.479 |

Prediction run:

```text
kubectl apply -f k8s/kernel-operator-qwen05-predict-job.yaml
```

Prediction output:

```text
/tmp/kernel-operator-qwen05-predictions
```

Policy evaluation:

```text
cargo run -p rehydration-testkit --bin kernel_operator_policy_eval -- \
  --trajectories /tmp/kernel-operator-sft-100/eval_trajectories.jsonl \
  --predictions /tmp/kernel-operator-qwen05-predictions/predictions.jsonl \
  --output /tmp/kernel-operator-qwen05-policy-eval.json
```

Held-out eval result:

| Predictor | Total | Exact | Tool | Ref | Scope | Stop | Invalid | Unbounded |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Qwen 0.5B LoRA operator | 291 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 0 | 0 |
| Deterministic baseline | 291 | 0.515 | 1.000 | 1.000 | 1.000 | 1.000 | 0 | 0 |

Interpretation:

- the first operator model learned the current read-navigation policy exactly
  on the held-out split;
- the task is learnable by a very small model;
- this does not yet prove generalization beyond the MemoryArena
  progressive-search trajectory distribution;
- next held-out cuts should split by task id/run family, not only random
  trajectory rows.

### P1.5 Ref-Safe Dataset Hardening

The first LoRA result was useful, but the row split was too easy and the raw
refs were not acceptable for a serious training claim.

The grouped V2 training attempt was stopped intentionally after this was found.
V2 should not be used as a reported result.

Raw MemoryArena refs encode more than identity:

```text
...:task:82:subtask:5:question
...:task:82:subtask:5:answer
```

That naming can leak role, temporal position, and benchmark structure. Even if
the model never sees benchmark answers, it could learn to choose refs from the
shape of the string rather than from KMP visible state. That is not the model
we want.

The preparer now supports two hardening flags:

```text
--anonymize-refs
--require-visible-target-refs
```

`--anonymize-refs` rewrites the model-facing trajectory with stable synthetic
refs per decision step:

```text
about/current_ref/known_refs/target_action refs -> ref_0001, ref_0002, ...
```

The original trajectories are still written for audit:

```text
train_trajectories.jsonl
eval_trajectories.jsonl
all_trajectories.jsonl
```

The anonymized trajectories are written for model evaluation:

```text
train_model_trajectories.jsonl
eval_model_trajectories.jsonl
all_model_trajectories.jsonl
```

Predictions from the anonymized dataset must be evaluated against
`eval_model_trajectories.jsonl`, not against the raw audit trajectories.

`--require-visible-target-refs` drops any row whose target action refers to a
ref that is not present in:

```text
current_ref
trace_target_ref
candidate_refs
known_refs
last_observed_refs
```

This matters because the first writer-read export exposed a real issue: some
read-before-write `kernel_near` targets were only recoverable from the raw ref
naming pattern, not from visible state. Those rows are not valid training
examples for an operator model.

Strict dataset command:

```text
python scripts/operator/prepare_operator_sft_dataset.py \
  --trajectories /tmp/kernel-operator-trajectories-100-with-writer/trajectories.jsonl \
  --output /tmp/kernel-operator-sft-100-with-writer-by-task-anon-visible \
  --split-mode group \
  --group-key task_id \
  --eval-ratio 0.1 \
  --seed 42 \
  --anonymize-refs \
  --require-visible-target-refs \
  --force
```

Real output:

| Dataset | Source rows | Kept | Dropped non-visible refs | Train | Eval | Train groups | Eval groups |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 100-task read + writer reads, grouped by task, anonymized refs | 5,724 | 4,318 | 1,406 | 3,854 | 464 | 89 | 11 |

Kept rows by mode:

| Mode | Rows |
| --- | ---: |
| `read` | 2,912 |
| `write_context_read` | 1,406 |

Kept target actions:

| Target action | Rows |
| --- | ---: |
| `kernel_near` | 753 |
| `kernel_inspect` | 2,159 |
| `kernel_trace` | 653 |
| `stop` | 753 |

The dropped rows are currently the first writer context-read step where the
target ref is not yet visible. That means the next writer exporter improvement
is clear: if a writer is allowed to inspect a candidate, the candidate must be
explicitly present in visible state. Otherwise the row is not trainable without
leaking naming conventions.

Leak audit for model-facing messages:

```text
jq -c 'select((.messages|map(.content)|join("\n")|test("memoryarena:|:question|:answer|:subtask:|:task:|target_action|observed_outcome|quality|gold|answer_session|has_answer")))' \
  /tmp/kernel-operator-sft-100-with-writer-by-task-anon-visible/train.jsonl \
  /tmp/kernel-operator-sft-100-with-writer-by-task-anon-visible/eval.jsonl \
  /tmp/kernel-operator-sft-100-with-writer-by-task-anon-visible/openai_train.jsonl \
  /tmp/kernel-operator-sft-100-with-writer-by-task-anon-visible/openai_eval.jsonl
```

returned no model-facing matches.

The local trainer should use the messages-only files for training:

```text
openai_train.jsonl
openai_eval.jsonl
```

The prediction script still uses `eval.jsonl`, because it needs `step_id` to
write evaluator-compatible predictions. `step_id` is metadata for the runner,
not part of the prompt.

Evaluator sanity checks on the anonymized eval set:

| Predictor | Total | Exact | Tool | Ref | Scope | Stop | Invalid | Unbounded |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Oracle baseline | 464 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 0 | 0 |
| Deterministic baseline | 464 | 0.349 | 1.000 | 0.606 | 1.000 | 1.000 | 0 | 0 |

The deterministic baseline losing ref accuracy after anonymization is expected
and useful: it shows that simple rules can no longer benefit from raw ref
naming. The next trained operator should beat this baseline by using visible
state, not benchmark-shaped ids.

Next V3 training rule:

- train on `openai_train.jsonl` / `openai_eval.jsonl`;
- predict on `eval.jsonl`;
- evaluate predictions against `eval_model_trajectories.jsonl`;
- keep `eval_trajectories.jsonl` only for audit against the original kernel
  refs.

V3 ref-safe trained operator result:

| Item | Value |
| --- | --- |
| Base model | `Qwen/Qwen2.5-0.5B-Instruct` |
| Adapter | `/tmp/kernel-operator-qwen05-lora-v3` |
| Kubernetes train job | `kernel-operator-qwen05-lora-v3` |
| Kubernetes predict job | `kernel-operator-qwen05-predict-v3` |
| Train duration | 25m Kubernetes job duration; 1,476s trainer runtime |
| Predict duration | 3m24s Kubernetes job duration with `--batch-size 8` |
| Train rows | 3,854 |
| Eval rows | 464 |
| Prediction failures | 0 |

Training metrics:

| Metric | Value |
| --- | ---: |
| `train_loss` | 0.06846 |
| `eval_loss` | 0.01203 |
| `eval_mean_token_accuracy` | 0.995 |
| `train_samples_per_second` | 7.834 |
| `train_steps_per_second` | 0.49 |

Held-out ref-safe eval result:

| Predictor | Total | Exact | Tool | Ref | Scope | Stop | Invalid | Unbounded |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Oracle baseline | 464 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 0 | 0 |
| Deterministic baseline | 464 | 0.349 | 1.000 | 0.606 | 1.000 | 1.000 | 0 | 0 |
| Qwen 0.5B LoRA V3 | 464 | 0.996 | 1.000 | 0.995 | 1.000 | 1.000 | 0 | 0 |

The two V3 misses are both in `write_context_read`. In both cases the model
selected the correct tool, bounded scope, and action type, but chose a different
visible `kernel_inspect` ref from the same candidate set. There were no missing
predictions, invalid JSON actions, unbounded tool calls, wrong tools, wrong
scopes, or wrong stop actions.

This is the first credible operator-model result because:

- benchmark-shaped raw refs are not visible to the prompt;
- train/eval is grouped by task;
- target refs must be visible to the model;
- evaluation is against anonymized model trajectories, not the raw audit
  trajectories.

Remaining gaps after V3:

- batched prediction is now acceptable for offline replay, but still not a live
  serving path: 464 rows took 3m24s with `--batch-size 8`, including dependency
  installation and model load;
- 1,406 writer-read rows were intentionally dropped because the target ref was
  not visible to the prompt; the exporter must expose valid candidates rather
  than relying on raw ref shape;
- the result is held out by task, but still from one 100-task MemoryArena run;
  the next credible step is a larger run and a family-held-out split;
- live MCP validation is still separate from offline policy replay;
- writer relation proposals should remain disabled until the read operator is
  validated at larger scale.

### P1.6 Candidate-Visible Writer Hardening

The V3 result was credible, but it exposed a dataset construction gap: 1,406
writer-read rows were dropped because their target refs were valid candidates
for the writer but were not present in the model-facing visible state. That is
not acceptable for a training claim. If the operator is allowed to choose a ref,
that ref must be visible without relying on raw naming conventions.

The exporter now adds `visible_state.candidate_refs` for writer context-read
steps. Candidate refs are gathered from:

- writer read context inspected refs;
- writer read context temporal refs;
- planned relation targets;
- relation-quality targets;
- primary refs already present in the recorded pre-read tool calls.

The current entry ref is excluded. The preparer now treats `candidate_refs` as
part of the visible ref set for strict target-ref validation and prompt
compaction.

Candidate-visible export:

```text
cargo run -p rehydration-testkit --bin kernel_operator_trajectory_export -- \
  --run /tmp/memoryarena-smart-writer-paged-100tasks-20260508-1407-run \
  --output /tmp/kernel-operator-trajectories-100-with-writer-candidates \
  --include-writer-reads \
  --expected-run-id smart-writer-paged-100tasks-20260508-1407 \
  --force
```

Strict candidate-visible dataset:

```text
python scripts/operator/prepare_operator_sft_dataset.py \
  --trajectories /tmp/kernel-operator-trajectories-100-with-writer-candidates/trajectories.jsonl \
  --output /tmp/kernel-operator-sft-100-with-writer-by-task-anon-visible-candidates \
  --split-mode group \
  --group-key task_id \
  --eval-ratio 0.1 \
  --seed 42 \
  --anonymize-refs \
  --require-visible-target-refs \
  --force
```

Real output:

| Dataset | Source rows | Kept | Dropped non-visible refs | Train | Eval | Train groups | Eval groups |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 100-task read + writer reads, grouped by task, anonymized refs, candidate-visible | 5,724 | 5,724 | 0 | 5,109 | 615 | 89 | 11 |

Rows by mode:

| Mode | Rows |
| --- | ---: |
| `read` | 2,912 |
| `write_context_read` | 2,812 |

Target actions:

| Target action | Rows |
| --- | ---: |
| `kernel_near` | 2,159 |
| `kernel_inspect` | 2,159 |
| `kernel_trace` | 653 |
| `stop` | 753 |

Eval-set baselines:

| Predictor | Total | Exact | Tool | Ref | Scope | Stop | Invalid | Unbounded |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Oracle baseline | 615 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 0 | 0 |
| Deterministic baseline | 615 | 0.263 | 1.000 | 0.434 | 1.000 | 1.000 | 0 | 0 |

V4 trained operator result:

| Item | Value |
| --- | --- |
| Base model | `Qwen/Qwen2.5-0.5B-Instruct` |
| Adapter | `/tmp/kernel-operator-qwen05-lora-v4` |
| Kubernetes train job | `kernel-operator-qwen05-lora-v4` |
| Kubernetes predict job | `kernel-operator-qwen05-predict-v4` |
| Train duration | 32m Kubernetes job duration; 1,930s trainer runtime |
| Predict duration | 5m14s Kubernetes job duration with `--batch-size 8` |
| Train rows | 5,109 |
| Eval rows | 615 |
| Prediction failures | 0 |

Training metrics:

| Metric | Value |
| --- | ---: |
| `train_loss` | 0.05677 |
| `eval_loss` | 0.01068 |
| `eval_mean_token_accuracy` | 0.9952 |
| `train_samples_per_second` | 7.942 |
| `train_steps_per_second` | 0.497 |

Held-out candidate-visible eval result:

| Predictor | Total | Exact | Tool | Ref | Scope | Stop | Invalid | Unbounded |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Oracle baseline | 615 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 0 | 0 |
| Deterministic baseline | 615 | 0.263 | 1.000 | 0.434 | 1.000 | 1.000 | 0 | 0 |
| Qwen 0.5B LoRA V4 | 615 | 0.993 | 1.000 | 0.993 | 1.000 | 1.000 | 0 | 0 |

The four V4 misses are all in `write_context_read` for the same MemoryArena
task family slice. They use the correct action type, tool, bounded arguments,
scope, and stop behavior, but choose a different visible candidate ref. This is
not a leak or boundedness issue. It points to the next dataset/modeling gap:
candidate refs are visible, but still unranked and mostly untyped. A stronger
writer-operator dataset should expose candidate roles, relation intent, or
writer evidence summaries rather than only a flat candidate ref list.

### P1.7 Writer Candidate Details

The next hardening step is to make writer candidates understandable without
leaking the writer's final relation decision.

The exporter now adds `visible_state.candidate_ref_details` beside
`candidate_refs`. Each detail is deliberately structural:

```json
{
  "ref": "ref_0002",
  "role": "same_subtask_question",
  "turn_kind": "question",
  "relative_position": "same_subtask",
  "temporal_distance": 0,
  "priority": 10,
  "relation_hint": "answer_addresses_question",
  "sources": ["writer_candidate_pool"]
}
```

What is intentionally not included:

- final `connect_to.rel`;
- final relation `why`;
- final relation evidence;
- raw memory text;
- benchmark answer labels;
- source names that reveal which candidate became the recorded action.

This matters because a writer operator should learn how to choose between
visible candidates, not memorize the post-hoc relation emitted by the writer.
The current source is normalized as `writer_candidate_pool` to avoid turning
candidate provenance into a label leak. The useful signal is structural:
turn kind, relative temporal position, candidate role, and bounded priority.

Candidate-detail export:

```text
cargo run -p rehydration-testkit --bin kernel_operator_trajectory_export -- \
  --run /tmp/memoryarena-smart-writer-paged-100tasks-20260508-1407-run \
  --output /tmp/kernel-operator-trajectories-100-with-writer-candidate-details \
  --include-writer-reads \
  --expected-run-id smart-writer-paged-100tasks-20260508-1407 \
  --force
```

Strict candidate-detail dataset:

```text
python scripts/operator/prepare_operator_sft_dataset.py \
  --trajectories /tmp/kernel-operator-trajectories-100-with-writer-candidate-details/trajectories.jsonl \
  --output /tmp/kernel-operator-sft-100-with-writer-by-task-anon-visible-candidate-details \
  --split-mode group \
  --group-key task_id \
  --eval-ratio 0.1 \
  --seed 42 \
  --anonymize-refs \
  --require-visible-target-refs \
  --force
```

Real output:

| Dataset | Source rows | Kept | Dropped non-visible refs | Train | Eval | Train groups | Eval groups |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 100-task read + writer reads, grouped by task, anonymized refs, candidate details | 5,724 | 5,724 | 0 | 5,109 | 615 | 89 | 11 |

Leak audit returned no model-facing rows for raw MemoryArena refs, target
actions, observed outcomes, quality fields, benchmark gold labels, answer
session ids, or `has_answer`.

The V5 training run uses the same base model and split as V4, but with
candidate details in the model-facing visible state:

| Item | Value |
| --- | --- |
| Base model | `Qwen/Qwen2.5-0.5B-Instruct` |
| Adapter | `/tmp/kernel-operator-qwen05-lora-v5` |
| Kubernetes train job | `kernel-operator-qwen05-lora-v5` |
| Kubernetes predict job | `kernel-operator-qwen05-predict-v5` |
| Train duration | 35m27s Kubernetes job duration; 2,092s trainer runtime |
| Predict duration | 4m55s Kubernetes job duration with `--batch-size 8` |
| Train rows | 5,109 |
| Eval rows | 615 |
| Prediction failures | 0 |

Training metrics:

| Metric | Value |
| --- | ---: |
| `train_loss` | 0.05612 |
| `eval_loss` | 0.00966 |
| `eval_mean_token_accuracy` | 0.9957 |
| `train_samples_per_second` | 7.326 |
| `train_steps_per_second` | 0.459 |

Held-out candidate-detail eval result:

| Predictor | Total | Exact | Tool | Ref | Scope | Stop | Invalid | Unbounded |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Oracle baseline | 615 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 0 | 0 |
| Deterministic baseline | 615 | 0.263 | 1.000 | 0.434 | 1.000 | 1.000 | 0 | 0 |
| Qwen 0.5B LoRA V5 | 615 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 0 | 0 |

V5 closed the four V4 writer-context-read misses. The important change is not
more hidden answer signal; it is better visible structure around the candidate
refs. The model sees candidate role, turn kind, relative temporal position,
priority, and a relation hint, while the final relation decision and evidence
remain hidden.

### P1.8 Explicit Holdout Validation

V5 proved that candidate details closed the remaining writer-context-read
misses, but the eval split was still produced by a seeded ratio over task
groups. P1.8 makes the holdout explicit and larger so the result is easier to
repeat and audit.

The SFT preparer now supports explicit group holdouts:

```text
--eval-group-values
--eval-group-values-file
```

It also supports additional group keys:

```text
task_id
task_type
task_family
mode
about
run_id
```

The V6 validation reserved task ids `80` through `99` for evaluation:

```text
python scripts/operator/prepare_operator_sft_dataset.py \
  --trajectories /tmp/kernel-operator-trajectories-100-with-writer-candidate-details/trajectories.jsonl \
  --output /tmp/kernel-operator-sft-100-with-writer-by-task-anon-visible-candidate-details-holdout20 \
  --split-mode group \
  --group-key task_id \
  --eval-group-values-file /tmp/kernel-operator-holdout20-groups.txt \
  --anonymize-refs \
  --require-visible-target-refs \
  --force
```

Real output:

| Dataset | Source rows | Train | Eval | Train groups | Eval groups | Dropped non-visible refs | Eval group values |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| 100-task read + writer reads, grouped by explicit task holdout, anonymized refs, candidate details | 5,724 | 4,600 | 1,124 | 80 | 20 | 0 | `80`-`99` |

Eval-set composition:

| Mode | Rows |
| --- | ---: |
| `read` | 572 |
| `write_context_read` | 552 |

| Target action | Rows |
| --- | ---: |
| `kernel_near` | 424 |
| `kernel_inspect` | 424 |
| `kernel_trace` | 128 |
| `stop` | 148 |

Leak audit returned no model-facing rows for raw MemoryArena refs, target
actions, observed outcomes, quality fields, benchmark gold labels, answer
session ids, or `has_answer`.

Eval-set baselines:

| Predictor | Total | Exact | Tool | Ref | Scope | Stop | Invalid | Unbounded |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Oracle baseline | 1,124 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 0 | 0 |
| Deterministic baseline | 1,124 | 0.263 | 1.000 | 0.434 | 1.000 | 1.000 | 0 | 0 |

V6 explicit-holdout trained operator result:

| Item | Value |
| --- | --- |
| Base model | `Qwen/Qwen2.5-0.5B-Instruct` |
| Adapter | `/tmp/kernel-operator-qwen05-lora-v6-holdout20` |
| Kubernetes train job | `kernel-operator-qwen05-lora-v6-holdout20` |
| Kubernetes predict job | `kernel-operator-qwen05-predict-v6-holdout20` |
| Train duration | 33m01s Kubernetes job duration; 1,946s trainer runtime |
| Predict duration | 8m50s Kubernetes job duration with `--batch-size 8` |
| Train rows | 4,600 |
| Eval rows | 1,124 |
| Prediction failures | 0 |

Training metrics:

| Metric | Value |
| --- | ---: |
| `train_loss` | 0.0588 |
| `eval_loss` | 0.01425 |
| `eval_mean_token_accuracy` | 0.9954 |
| `train_samples_per_second` | 7.092 |
| `train_steps_per_second` | 0.444 |

Held-out explicit-task eval result:

| Predictor | Total | Exact | Tool | Ref | Scope | Stop | Invalid | Unbounded |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Oracle baseline | 1,124 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 0 | 0 |
| Deterministic baseline | 1,124 | 0.263 | 1.000 | 0.434 | 1.000 | 1.000 | 0 | 0 |
| Qwen 0.5B LoRA V6 holdout20 | 1,124 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 0 | 0 |

This does not prove cross-benchmark generalization or live MCP serving
robustness. It does prove that the operator is not only memorizing the smaller
V5 split: with benchmark-shaped refs hidden, it generalizes across a larger
explicit held-out task range inside the same MemoryArena run.

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
