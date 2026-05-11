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
writer candidates visible and the strict split dropped zero rows. V5 is the
preferred claim because it adds structural candidate details and closes the
remaining writer-context-read ref selection misses without exposing final writer
relations.

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
jq -c 'select((.messages|map(.content)|join("\n")|test("memoryarena:|:question|:answer|:subtask:|:task:|target_action|observed_outcome|quality|gold|answer_session|has_answer")))' \
  /tmp/kernel-operator-sft-100-with-writer-by-task-anon-visible-candidate-details/train.jsonl \
  /tmp/kernel-operator-sft-100-with-writer-by-task-anon-visible-candidate-details/eval.jsonl \
  /tmp/kernel-operator-sft-100-with-writer-by-task-anon-visible-candidate-details/openai_train.jsonl \
  /tmp/kernel-operator-sft-100-with-writer-by-task-anon-visible-candidate-details/openai_eval.jsonl
```

Expected output: no rows.

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
