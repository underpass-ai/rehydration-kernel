# Kernel Operator Training

This folder contains the external training path for a small KMP tool-operator
model. The model is not part of kernel core. It learns to emit one bounded
KMP/MCP action from a visible memory state.

## 1. Prepare SFT Data

```bash
python scripts/operator/prepare_operator_sft_dataset.py \
  --trajectories /tmp/kernel-operator-trajectories-100/trajectories.jsonl \
  --output /tmp/kernel-operator-sft-100 \
  --eval-ratio 0.1 \
  --seed 42 \
  --force
```

Harder split by task:

```bash
python scripts/operator/prepare_operator_sft_dataset.py \
  --trajectories /tmp/kernel-operator-trajectories-100-with-writer/trajectories.jsonl \
  --output /tmp/kernel-operator-sft-100-with-writer-by-task \
  --split-mode group \
  --group-key task_id \
  --eval-ratio 0.1 \
  --seed 42 \
  --force
```

Use the grouped split for model claims. The row split is useful only for smoke
tests because it can place adjacent steps from the same task in both train and
eval.

Strict ref-safe split for the current real operator run:

```bash
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

Use this mode for training claims. It replaces model-facing refs with synthetic
per-step refs such as `ref_0001`, and drops rows whose target action refers to
refs that are not visible in `current_ref`, `trace_target_ref`,
`candidate_refs`, `candidate_ref_details`, `known_refs`, or
`last_observed_refs`. `candidate_refs` is required for writer context-read rows;
without it, valid writer candidates can look invisible after anonymization.

The current preferred dataset also includes `candidate_ref_details` for writer
context-read rows. These details are structural and model-facing after
anonymization: role, turn kind, relative temporal position, priority, and a
relation hint derived from the entry kind. They intentionally do not expose the
writer's final `connect_to.rel`, `why`, evidence text, or source names that
would reveal the recorded target action.

The previous grouped V2 training attempt was stopped after this issue was
identified. V3 fixed the reporting path by dropping non-visible refs. V4 made
writer candidates visible and the strict split dropped zero rows. V5 adds
structural candidate details and closes the remaining writer-context-read ref
selection misses without exposing final writer relations. V6 is the preferred
validation claim because it repeats the candidate-detail setup with a larger
explicit holdout of task ids `80` through `99`.

Explicit holdout split:

```bash
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

Outputs:

- `train.jsonl`
- `eval.jsonl`
- `all.jsonl`
- `train_trajectories.jsonl`
- `eval_trajectories.jsonl`
- `all_trajectories.jsonl`
- `train_model_trajectories.jsonl`
- `eval_model_trajectories.jsonl`
- `all_model_trajectories.jsonl`
- `dropped_non_visible_target_refs.jsonl`
- `summary.json`

The user prompt excludes target actions, observed outcomes, benchmark gold
answers, and hidden raw memory.

For strict anonymized datasets:

- `*_trajectories.jsonl` keeps original refs for audit;
- `*_model_trajectories.jsonl` keeps anonymized refs for evaluation;
- predictions from anonymized prompts must be evaluated against
  `eval_model_trajectories.jsonl`;
- local SFT training should use `openai_train.jsonl` and `openai_eval.jsonl`
  because those files contain only `messages`.

Prompt leak audit:

```bash
python scripts/operator/audit_operator_sft_no_gold.py \
  /tmp/kernel-operator-sft-100-with-writer-by-task-anon-visible-candidate-details/openai_train.jsonl \
  /tmp/kernel-operator-sft-100-with-writer-by-task-anon-visible-candidate-details/openai_eval.jsonl \
  --output /tmp/kernel-operator-sft-100-with-writer-by-task-anon-visible-candidate-details/no-gold-audit.json
```

Expected result: `finding_count` is `0`.

### LongMemEval Trajectories

LongMemEval uses a separate exporter. Do not route LongMemEval rows through the
MemoryArena exporter; both exporters emit the same
`kernel-operator-trajectory-v1` contract so downstream preparation can consume
them together.

```bash
cargo run -p rehydration-testkit --bin longmemeval_operator_trajectory_export -- \
  --run <longmemeval-run-dir> \
  --artifacts <longmemeval-adapter-artifacts-dir> \
  --output <longmemeval-operator-trajectories-dir> \
  --expected-run-id <run-id> \
  --force
```

For LongMemEval smart-writer runs, include writer context reads:

```bash
cargo run -p rehydration-testkit --bin longmemeval_operator_trajectory_export -- \
  --run <longmemeval-smart-writer-run-dir> \
  --artifacts <longmemeval-smart-writer-artifacts-dir> \
  --output <longmemeval-smart-writer-operator-trajectories-dir> \
  --expected-run-id <run-id> \
  --include-writer-reads \
  --force
```

Mixed MemoryArena + LongMemEval SFT data is prepared by passing multiple
trajectory files. Keep `--split-mode group --group-key task_id`: MemoryArena
groups by task id, LongMemEval groups by question id, and writer rows use the
same logical question id.

```bash
python scripts/operator/prepare_operator_sft_dataset.py \
  --trajectories <memoryarena-operator-trajectories>/trajectories.jsonl \
  --trajectories <longmemeval-operator-trajectories>/trajectories.jsonl \
  --trajectories <longmemeval-smart-writer-operator-trajectories>/trajectories.jsonl \
  --output <mixed-operator-sft-dir> \
  --split-mode group \
  --group-key task_id \
  --anonymize-refs \
  --require-visible-target-refs \
  --force
```

## 2. Train LoRA

Strict next run:

```bash
python scripts/operator/train_operator_sft_lora.py \
  --train-jsonl /tmp/kernel-operator-sft-100-with-writer-by-task-anon-visible-candidate-details/openai_train.jsonl \
  --eval-jsonl /tmp/kernel-operator-sft-100-with-writer-by-task-anon-visible-candidate-details/openai_eval.jsonl \
  --model-id Qwen/Qwen2.5-0.5B-Instruct \
  --output-dir /tmp/kernel-operator-qwen05-lora-v5 \
  --epochs 3 \
  --batch-size 2 \
  --grad-accum 8 \
  --max-length 2048 \
  --bf16
```

Use `--fp16` instead of `--bf16` if the GPU does not support bfloat16.

## 3. Predict

```bash
python scripts/operator/predict_operator_sft.py \
  --dataset-jsonl /tmp/kernel-operator-sft-100-with-writer-by-task-anon-visible-candidate-details/eval.jsonl \
  --model-id Qwen/Qwen2.5-0.5B-Instruct \
  --adapter /tmp/kernel-operator-qwen05-lora-v5 \
  --output /tmp/kernel-operator-qwen05-predictions-v5 \
  --batch-size 8 \
  --force
```

## 4. Evaluate

```bash
cargo run -p rehydration-testkit --bin kernel_operator_policy_eval -- \
  --trajectories /tmp/kernel-operator-sft-100-with-writer-by-task-anon-visible-candidate-details/eval_model_trajectories.jsonl \
  --predictions /tmp/kernel-operator-qwen05-predictions-v5/predictions.jsonl \
  --output /tmp/kernel-operator-qwen05-predictions-v5-policy-eval.json
```

For non-anonymized smoke datasets, `eval_model_trajectories.jsonl` and
`eval_trajectories.jsonl` are equivalent. For strict anonymized datasets, always
use `eval_model_trajectories.jsonl`.

The Kubernetes prediction job may create `/tmp/kernel-operator-qwen05-predictions-v5`
as `nobody`. In that case, write `policy-eval.json` to a sibling path as shown
above.

The main comparison is against:

- deterministic baseline;
- OpenAI generalist baseline;
- small trained operator.

Observed V3 ref-safe run on 2026-05-11 with `--batch-size 8`:

| Predictor | Total | Exact | Tool | Ref | Scope | Stop | Invalid | Unbounded |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Oracle baseline | 464 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 0 | 0 |
| Deterministic baseline | 464 | 0.349 | 1.000 | 0.606 | 1.000 | 1.000 | 0 | 0 |
| Qwen 0.5B LoRA V3 | 464 | 0.996 | 1.000 | 0.995 | 1.000 | 1.000 | 0 | 0 |

The V3 run produced 464 predictions with zero parse failures. The only two
exact mismatches used the correct tool and bounded arguments but selected a
different visible `kernel_inspect` ref in writer-context-read steps.

The batched Kubernetes prediction job completed in 3m24s including dependency
installation, model load, and generation. The previous unbatched path took 16m
for the same 464 rows.

Observed V4 candidate-visible run on 2026-05-11 with `--batch-size 8`:

| Predictor | Total | Exact | Tool | Ref | Scope | Stop | Invalid | Unbounded |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Oracle baseline | 615 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 0 | 0 |
| Deterministic baseline | 615 | 0.263 | 1.000 | 0.434 | 1.000 | 1.000 | 0 | 0 |
| Qwen 0.5B LoRA V4 | 615 | 0.993 | 1.000 | 0.993 | 1.000 | 1.000 | 0 | 0 |

V4 trained on 5,109 rows and evaluated on 615 rows, grouped by task with
synthetic model-facing refs. Prediction produced 615 rows with zero parse
failures and completed in 5m14s including dependency installation and model
load. The four exact misses are all writer-context-read choices where the model
selected a different visible candidate ref with the correct tool, scope, and
bounded arguments.

V5 candidate-detail run:

```bash
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

This dataset keeps the same 5,109 train rows and 615 eval rows as V4, with zero
dropped non-visible target refs. Use `kernel-operator-qwen05-lora-v5` and
`kernel-operator-qwen05-predict-v5` for the Kubernetes LoRA/prediction jobs.

Observed V5 candidate-detail run on 2026-05-11 with `--batch-size 8`:

| Predictor | Total | Exact | Tool | Ref | Scope | Stop | Invalid | Unbounded |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Oracle baseline | 615 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 0 | 0 |
| Deterministic baseline | 615 | 0.263 | 1.000 | 0.434 | 1.000 | 1.000 | 0 | 0 |
| Qwen 0.5B LoRA V5 | 615 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 0 | 0 |

V5 trained on 5,109 rows and evaluated on 615 rows, grouped by task with
synthetic model-facing refs. Prediction produced 615 rows with zero parse
failures and completed in 4m55s including dependency installation and model
load. The training job completed in 35m27s, with final `eval_loss` 0.00966 and
`eval_mean_token_accuracy` 0.9957.

V6 explicit-holdout run:

```bash
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

The holdout file reserves task ids `80` through `99` for eval. The split
contains 4,600 train rows and 1,124 eval rows, with zero dropped non-visible
target refs.

Observed V6 explicit-holdout run on 2026-05-11 with `--batch-size 8`:

| Predictor | Total | Exact | Tool | Ref | Scope | Stop | Invalid | Unbounded |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Oracle baseline | 1,124 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 0 | 0 |
| Deterministic baseline | 1,124 | 0.263 | 1.000 | 0.434 | 1.000 | 1.000 | 0 | 0 |
| Qwen 0.5B LoRA V6 holdout20 | 1,124 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 0 | 0 |

V6 trained on 4,600 rows and evaluated on 1,124 rows. Prediction produced
1,124 rows with zero parse failures. The training job completed in 33m01s, with
final `eval_loss` 0.01425 and `eval_mean_token_accuracy` 0.9954. The prediction
job completed in 8m50s including dependency installation and model load.

## 5. De-Anonymize Predictions For Raw Replay

Predictions from strict anonymized datasets contain synthetic refs such as
`ref_0001`. They are correct for offline model evaluation, but they cannot be
executed against a live kernel until those refs are mapped back to raw kernel
refs.

Use the paired raw/model trajectory files to create evaluator-compatible raw
predictions:

```bash
python scripts/operator/deanonymize_operator_predictions.py \
  --raw-trajectories /tmp/kernel-operator-sft-100-with-writer-by-task-anon-visible-candidate-details-holdout20/eval_trajectories.jsonl \
  --model-trajectories /tmp/kernel-operator-sft-100-with-writer-by-task-anon-visible-candidate-details-holdout20/eval_model_trajectories.jsonl \
  --predictions /tmp/kernel-operator-qwen05-predictions-v6-holdout20/predictions.jsonl \
  --output /tmp/kernel-operator-qwen05-predictions-v6-holdout20-raw \
  --force
```

Outputs:

- `predictions.jsonl`: raw-ref predictions accepted by
  `kernel_operator_policy_eval`;
- `audit.jsonl`: one row per prediction with model action, raw action, and
  mapped synthetic refs;
- `failures.jsonl`: missing or unmappable refs;
- `summary.json`: selected/written/failure counts.

Fail-fast behavior is intentional. If a predicted synthetic ref is not visible
in the paired model/raw trajectory, the row is rejected instead of inventing a
mapping.

Raw-ref evaluation:

```bash
cargo run -p rehydration-testkit --bin kernel_operator_policy_eval -- \
  --trajectories /tmp/kernel-operator-sft-100-with-writer-by-task-anon-visible-candidate-details-holdout20/eval_trajectories.jsonl \
  --predictions /tmp/kernel-operator-qwen05-predictions-v6-holdout20-raw/predictions.jsonl \
  --output /tmp/kernel-operator-qwen05-predictions-v6-holdout20-raw-policy-eval.json
```

Observed V6 de-anonymization result on 2026-05-11:

| Item | Value |
| --- | ---: |
| Selected predictions | 1,124 |
| Written raw predictions | 1,124 |
| Failures | 0 |
| Mapped synthetic refs | 5,240 |

Raw-ref policy eval stayed exact:

| Predictor | Total | Exact | Tool | Ref | Scope | Stop | Invalid | Unbounded |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Qwen 0.5B LoRA V6 holdout20, de-anonymized | 1,124 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 0 | 0 |

## 6. Replay Raw Predictions Against Live MCP

Raw-ref policy eval proves the predicted action matches the audited target
action. Live replay proves the predicted action is executable against the
kernel through the real MCP adapter and typed gRPC service.

Use `kernel_operator_mcp_replay` after de-anonymization:

```bash
cargo run -p rehydration-testkit --bin kernel_operator_mcp_replay -- \
  --trajectories /tmp/kernel-operator-sft-100-with-writer-by-task-anon-visible-candidate-details-holdout20/eval_trajectories.jsonl \
  --predictions /tmp/kernel-operator-qwen05-predictions-v6-holdout20-raw/predictions.jsonl \
  --output /tmp/kernel-operator-qwen05-predictions-v6-holdout20-mcp-replay-100 \
  --endpoint https://rehydration-kernel.underpassai.com \
  --limit 100 \
  --log-progress-every 25 \
  --force
```

Outputs:

- `results.jsonl`: one row per trajectory step with action, tool result,
  observed refs, missing expected refs, and extra observed refs;
- `summary.json`: selected rows, tool calls, stop actions, boundedness failures,
  MCP failures, ref coverage, action mix, and latency by action.

For long runs, `--log-progress-every N` writes compact JSONL progress events to
stderr without changing the replay result files.

The replay fails fast when:

- a prediction is missing;
- a prediction is malformed;
- a tool call is unbounded;
- MCP/gRPC returns an error;
- a tool call does not return the refs observed in the audited trajectory.

Observed 100-step live smoke on 2026-05-11:

| Item | Value |
| --- | ---: |
| Selected trajectory steps | 100 |
| Executed tool calls | 85 |
| Stop actions | 15 |
| Successful tool calls | 85 |
| Failed tool calls | 0 |
| Missing expected ref rows | 0 |
| Unbounded tool calls | 0 |

Observed full V6 holdout20 live replay on 2026-05-11:

| Item | Value |
| --- | ---: |
| Selected trajectory steps | 1,124 |
| Executed tool calls | 976 |
| Stop actions | 148 |
| Successful tool calls | 976 |
| Failed tool calls | 0 |
| Missing expected ref rows | 0 |
| Missing predictions | 0 |
| Invalid predictions | 0 |
| Unbounded tool calls | 0 |
| Extra observed ref rows | 848 |
| Elapsed | 7m18.7s |

Extra observed refs mean the live kernel returned additional valid context
beyond the audited minimum. The replay fails only when expected refs are
missing.

Full replay action latency against the public TLS endpoint:

| Action | Count | Avg ms | Max ms |
| --- | ---: | ---: | ---: |
| `kernel_near` | 424 | 922.2 | 1,738 |
| `kernel_inspect` | 424 | 79.2 | 146 |
| `kernel_trace` | 128 | 105.9 | 168 |
| `stop` | 148 | 0.0 | 0 |

## 7. Next Scale Run

The validated claim today is the V6 explicit holdout20 run. The next publishable
operator cut should scale the same pipeline rather than changing model
semantics.

Run rules:

- top 1 gate: bounded pagination/progress/resume for remote audit and replay
  must be validated before using a run as publication evidence;
- start from a fresh audited MemoryArena smart-writer run;
- generate a fresh `run_id` for every live run or smoke;
- split by task id or run family, never by individual trajectory row;
- keep `--anonymize-refs` and `--require-visible-target-refs`;
- use raw refs only after prediction, through de-anonymization;
- run live MCP replay only after offline policy eval has zero invalid and
  unbounded actions.

Do not reuse the same `run_id` for a second live smoke. The deployed kernel is
append/projection based; previous writes under the same `about` can make early
asks observe answer feedback from an earlier attempt and create false
future-leak failures.

Recommended sequence:

First validate the P1.11.0 audit/replay pagination gate. The audit command must
emit progress by about/task and support resume before it is used as publication
evidence.

`memoryarena_kmp_run_audit` supports paged remote inspect through `--limit` and
`--offset`. It writes `inspect.next_offset` in the summary and emits JSONL
progress events to stderr and, optionally, `--progress-output`.

Temporal reads are also page-aware. `kernel_goto`, `kernel_near`,
`kernel_rewind`, `kernel_forward`, and `kernel_trace` expose a `page` object in
MCP structured output. Live replay writes that page into `results.jsonl`,
marks rows with `partial_result=true` when `page.has_more=true`, and reports
partial-result counts in `summary.json`.

```bash
cargo run -p rehydration-testkit --bin memoryarena_kmp_run_audit -- \
  --run <memoryarena-run-dir> \
  --endpoint <public-kernel-url> \
  --inspect \
  --expected-run-id <run-id> \
  --output <audit.json> \
  --limit 100 \
  --offset 0 \
  --log-progress-every 25 \
  --progress-output <audit-progress.jsonl> \
  --force

# For the next audit page, use inspect.next_offset from <audit.json> or the
# last progress event's next_offset as the new --offset.

cargo run -p rehydration-testkit --bin kernel_operator_trajectory_export -- \
  --run <memoryarena-run-dir> \
  --output <operator-trajectories-dir> \
  --include-writer-reads \
  --force

python scripts/operator/prepare_operator_sft_dataset.py \
  --trajectories <operator-trajectories-dir>/trajectories.jsonl \
  --output <operator-sft-dir> \
  --split-mode group \
  --group-key task_id \
  --anonymize-refs \
  --require-visible-target-refs \
  --force

python scripts/operator/train_operator_sft_lora.py \
  --train-jsonl <operator-sft-dir>/openai_train.jsonl \
  --eval-jsonl <operator-sft-dir>/openai_eval.jsonl \
  --model-id Qwen/Qwen2.5-0.5B-Instruct \
  --output-dir <operator-lora-dir> \
  --epochs 3 \
  --batch-size 2 \
  --grad-accum 8 \
  --max-length 2048 \
  --bf16

python scripts/operator/predict_operator_sft.py \
  --dataset-jsonl <operator-sft-dir>/eval.jsonl \
  --model-id Qwen/Qwen2.5-0.5B-Instruct \
  --adapter <operator-lora-dir> \
  --output <operator-predictions-dir> \
  --batch-size 8 \
  --force

cargo run -p rehydration-testkit --bin kernel_operator_policy_eval -- \
  --trajectories <operator-sft-dir>/eval_model_trajectories.jsonl \
  --predictions <operator-predictions-dir>/predictions.jsonl \
  --output <operator-policy-eval>.json
```

Only after that passes, de-anonymize and replay against live MCP as shown in
sections 5 and 6. Use `--limit 100` first; run the full replay only if the
smoke has zero missing predictions, invalid predictions, unbounded calls, MCP
failures, and missing expected refs.

## 8. Publication Packaging

Do not publish a model only from local accuracy. Package the release after the
P1.11 gate is clean:

- copy the model card template from
  `docs/product/huggingface/kernel-tool-operator-small-model-card-template.md`;
- copy the dataset card template from
  `docs/product/huggingface/kernel-operator-trajectories-dataset-card-template.md`;
- fill the release evaluation summary from
  `docs/product/huggingface/operator-release-eval-summary-template.md`;
- keep Hugging Face repos private first;
- verify download, local inference, offline eval, de-anonymization, and live MCP
  replay from the published artifacts;
- make the repos public only after that verification is clean.
