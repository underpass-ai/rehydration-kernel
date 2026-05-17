# 2026-05-17 Writer Orchestration V2 KMP Cursor Dataset

Status: dataset-ready
Date: 2026-05-17

Current preserved artifact root:

```text
../rehydration-kernel-artifacts/operator/2026-05-17-kmp-cursor-contract-refresh/
```

Use `/tmp` only as regeneration scratch space. Training, prediction, and
coverage gates must use the preserved artifact root.

## 1. Purpose

Close the gaps found after the first mixed writer-orchestration run before
launching another GPU training job.

The objective is unchanged: train Operator to decide one bounded writer-path
step:

- read more context with `kernel_near`, `kernel_trace`, or `kernel_inspect`;
- stop when the visible context is sufficient or the write must fail fast;
- execute an already prepared `kernel_write_memory` or `kernel_ingest` payload
  through deterministic prepared-payload resolution.

Operator still does not author semantic relations, `why`, evidence, or full
write payloads. Those are prepared by the writer/LLM or a stronger teacher.

## 2. Gaps Found In V1

The v1 training run is now quarantined. It is useful history, but it must not
be used as a product claim.

| Gap | Impact | V2 fix |
| --- | --- | --- |
| Symbolic `kernel_trace.page.cursor` values such as `trace:page:2` | The model could learn a cursor shape that real KMP does not return. | Trace continuation cursors are numeric strings matching KMP `Trace.next_cursor`. |
| Stop scoring only checked `type=stop` | A model could stop with wrong evidence and still receive stop credit. | Policy eval now requires exact `answer_policy` and `final_refs`. |
| `prepared_tool_call` was not fully validated in SFT preparation | The model-facing shortcut could drift from the real action resolver. | SFT preparation validates the full action contract and verifies the visible payload matches `target_action.arguments`. |
| Predictor accepted invalid trace cursor and `source_kind` shapes | Generated predictions could pass locally and fail against real KMP/MCP. | Predictor and Rust validator reject nonnumeric trace cursors and unsupported source kinds. |
| Operator and some KMP/MCP write paths accepted noncanonical relation aliases | A model or client could emit fuzzy relation names and still pass after silent canonicalization. | Operator validation and public KMP/MCP write boundaries now require canonical relation wire values. |
| Writer-exec ids were not namespaced by `about` | Repeated synthetic topics could collide in later replay or persistence. | Writer-exec abouts and idempotency keys include the run/topic namespace. |
| Writer pre-read trajectories declared the full read tool surface while the SFT profile projected to `near`/`trace`/`inspect` | The preparer could hide out-of-profile tools silently, making prompt/API parity harder to audit. | Writer pre-read export now emits profile-specific `allowed_tools`, and SFT preparation fails if a row contains tools outside the active profile. |

## 3. Source Trajectories

Sources:

```text
/tmp/kernel-operator-conformance-writer-pre-read-v4-kmp-cursor-v2-20260517/trajectories.jsonl
/tmp/kernel-operator-conformance-writer-exec-v1-kmp-cursor-v2-20260517/trajectories.jsonl
```

Source export summaries:

| Source | Rows | Modes | Target actions | Contract failures |
| --- | ---: | --- | --- | ---: |
| writer pre-read v4 KMP cursor v2 | 576 | `write_context_read` | stop 144, inspect 72, near 72, trace 288 | 0 |
| writer exec v1 KMP cursor v2 | 234 | `write` | stop 126, ingest 36, write_memory 72 | 0 |

## 4. Dataset Preparation

Output:

```text
../rehydration-kernel-artifacts/operator/2026-05-17-kmp-cursor-contract-refresh/kernel-operator-sft-writer-orchestration-v2-kmp-cursor-20260517
```

Command:

```text
SCRATCH_ROOT=/tmp/kernel-operator-sft-writer-orchestration-v2-kmp-cursor-20260517

python scripts/operator/prepare_operator_sft_dataset.py \
  --trajectories /tmp/kernel-operator-conformance-writer-pre-read-v4-kmp-cursor-v2-20260517/trajectories.jsonl \
  --trajectories /tmp/kernel-operator-conformance-writer-exec-v1-kmp-cursor-v2-20260517/trajectories.jsonl \
  --output "$SCRATCH_ROOT" \
  --include-mode write_context_read \
  --include-mode write \
  --eval-ratio 0.3 \
  --split-mode group \
  --group-key task_or_step \
  --capability-split-profile writer-orchestration \
  --min-train-capability-count 5 \
  --min-eval-capability-count 5 \
  --anonymize-refs \
  --require-visible-target-refs \
  --require-visible-target-cursors \
  --force
```

## 5. Dataset Evidence

| Metric | Value |
| --- | ---: |
| selected rows | 810 |
| train rows | 567 |
| eval rows | 243 |
| `write_context_read` rows | 576 |
| `write` rows | 234 |
| total capability coverage | 40 / 40 |
| train capability coverage | 40 / 40 |
| eval capability coverage | 40 / 40 |
| duplicate model-row hashes | 0 |
| train/eval model-row overlap | 0 |
| dropped non-visible target refs | 0 |
| dropped non-visible target cursors | 0 |

Model-facing action distribution:

| Action | Count |
| --- | ---: |
| `kernel_trace` | 288 |
| `stop` | 270 |
| `prepared_tool_call` | 108 |
| `kernel_near` | 72 |
| `kernel_inspect` | 72 |

Prompt/tool parity:

| Field | Value |
| --- | --- |
| prompt profile | `writer-orchestration` |
| target projection | `prepared_payload_decision_v1` |
| visible tools | `kernel_near`, `kernel_trace`, `kernel_inspect`, `kernel_write_memory`, `kernel_ingest` |
| forbidden visible tools | none |
| missing visible tools | none |

## 6. Oracle Gate

This gate was rerun after tightening Operator validation for exact
`source_kind`, `semantic_class`, `confidence`, `answer_policy`, `detail`,
canonical relation `rel` wire values, and profile-specific `allowed_tools`
parity.

Command:

```text
cargo run -p underpass-operator-evaluation-cli --bin underpass_operator_policy_eval -- \
  --model-facing-eval ../rehydration-kernel-artifacts/operator/2026-05-17-kmp-cursor-contract-refresh/kernel-operator-sft-writer-orchestration-v2-kmp-cursor-20260517/eval.jsonl \
  --baseline oracle \
  --resolve-prepared-payloads \
  --output ../rehydration-kernel-artifacts/operator/2026-05-17-kmp-cursor-contract-refresh/kernel-operator-sft-writer-orchestration-v2-kmp-cursor-20260517/oracle-policy-eval.json \
  --details-output ../rehydration-kernel-artifacts/operator/2026-05-17-kmp-cursor-contract-refresh/kernel-operator-sft-writer-orchestration-v2-kmp-cursor-20260517/oracle-policy-details.jsonl
```

Result:

| Metric | Value |
| --- | ---: |
| exact action | 243 / 243 |
| invalid predictions | 0 |
| unbounded tool calls | 0 |
| `write_context_read` exact | 181 / 181 |
| `write` exact | 62 / 62 |
| stop accuracy | 83 / 83 |
| trace continuation accuracy | 54 / 54 |
| cursor mode accuracy | 21 / 21 |
| window shape accuracy | 21 / 21 |
| limit policy accuracy | 21 / 21 |

Resolved eval final-action distribution:

| Final action | Count |
| --- | ---: |
| `kernel_trace` | 90 |
| `stop` | 83 |
| `kernel_inspect` | 23 |
| `kernel_near` | 21 |
| `kernel_write_memory` | 21 |
| `kernel_ingest` | 5 |

## 7. Kubernetes Jobs

Trainer preflight:

```text
RUN_ROOT=/tmp/kernel-operator-writer-orchestration-v2-preflight

python scripts/operator/train_operator_sft_lora.py \
  --train-jsonl ../rehydration-kernel-artifacts/operator/2026-05-17-kmp-cursor-contract-refresh/kernel-operator-sft-writer-orchestration-v2-kmp-cursor-20260517/openai_train.jsonl \
  --eval-jsonl ../rehydration-kernel-artifacts/operator/2026-05-17-kmp-cursor-contract-refresh/kernel-operator-sft-writer-orchestration-v2-kmp-cursor-20260517/openai_eval.jsonl \
  --output-dir "$RUN_ROOT/lora-validate-only" \
  --validate-only
```

Result: 567 train rows and 243 eval rows validated before loading training
dependencies.

Prediction preflight:

```text
python scripts/operator/predict_operator_sft.py \
  --dataset-jsonl ../rehydration-kernel-artifacts/operator/2026-05-17-kmp-cursor-contract-refresh/kernel-operator-sft-writer-orchestration-v2-kmp-cursor-20260517/eval.jsonl \
  --model-id Qwen/Qwen2.5-0.5B-Instruct \
  --output "$RUN_ROOT/predict-validate-only" \
  --resolve-prepared-payloads \
  --validate-only
```

Result: 243 eval rows validated before creating prediction output or loading
the model.

API/MCP contract coverage:

```text
cargo run -p underpass-operator-evaluation-cli --bin underpass_operator_contract_coverage -- \
  --profile writer-pre-read \
  --trajectories ../rehydration-kernel-artifacts/operator/2026-05-17-kmp-cursor-contract-refresh/kernel-operator-sft-writer-orchestration-v2-kmp-cursor-20260517/eval.jsonl \
  --fail-under 100

cargo run -p underpass-operator-evaluation-cli --bin underpass_operator_contract_coverage -- \
  --profile write \
  --trajectories ../rehydration-kernel-artifacts/operator/2026-05-17-kmp-cursor-contract-refresh/kernel-operator-sft-writer-orchestration-v2-kmp-cursor-20260517/eval.jsonl \
  --fail-under 100
```

Result: both profiles report 100% profile contract coverage and 100% target
capability coverage. The reporter resolves model-facing `prepared_tool_call`
rows into final `kernel_write_memory` / `kernel_ingest` actions before measuring
write coverage.

Training job:

```text
k8s/kernel-operator-qwen05-lora-writer-orchestration-v2-kmp-cursor-4gpu-20260517-job.yaml
```

Prediction job:

```text
k8s/kernel-operator-qwen05-predict-writer-orchestration-v2-kmp-cursor-20260517-job.yaml
```

The prediction job must use:

```text
--resolve-prepared-payloads
```

Live replay receives final resolved `kernel_write_memory` or `kernel_ingest`
actions. It should not receive `prepared_tool_call`, because that is an
Operator-side model-facing shortcut, not a public KMP/MCP tool.

## 8. Decision

This dataset is ready for the next controlled writer-orchestration training
run.

It is not yet a trained model result. Do not claim that the v2 model passed
until training, prediction, strict resolved policy eval, raw de-anonymized eval,
and live MCP replay have all been rerun against this v2 dataset.

## 9. Next Gates

Before promotion:

- train with the v2 KMP-cursor dataset;
- predict with `--resolve-prepared-payloads`;
- evaluate with `--model-facing-eval` and `--resolve-prepared-payloads`;
- de-anonymize predictions back to raw refs;
- run raw policy eval;
- replay final resolved actions through MCP/KMP;
- confirm stop rows require exact `final_refs`;
- confirm no `prepared_tool_call` reaches replay;
- confirm no nonnumeric trace cursor appears in predictions.
- confirm relation `rel` values are canonical wire values, not aliases.

## 10. Artifact Hashes

| Artifact | SHA-256 |
| --- | --- |
| SFT summary | `4fe0f17c25a98a7157c2f28547a051baa41e0ae586fc68ae673d0ae83b8a3227` |
| `train.jsonl` | `3cc50ea0f076949e7b004b61df92171a9463ad32813f2c04d7a542dcf05e8d4f` |
| `eval.jsonl` | `61b4d8b3abdc1c64aba7b7d64804a1ee6dd1cefd8d0a3c96db8f25d8ede37979` |
| `openai_train.jsonl` | `5069d2bc9bb8f1a325faf92d26500629f7a69240f0e6bb1dd81ced075f5bf4c6` |
| `openai_eval.jsonl` | `e4b62da25bdbc6dda3eee90b9cf87666fcada802ce87dd0acec92e22cb58b45b` |
| oracle policy eval | `57126a63128acb5822564a1ac8eae183ae9e3558a75e26e371e3ce7064d1e247` |
