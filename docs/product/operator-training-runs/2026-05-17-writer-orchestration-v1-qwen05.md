# 2026-05-17 Writer Orchestration V1 Qwen 0.5B Run

Status: quarantined
Date: 2026-05-17

Quarantine note, added after the KMP-cursor audit:

This run is retained as historical evidence only. It passed the strictness that
existed at the time, but the later writer-orchestration audit found three
contract gaps that make the result unsuitable for product or publication
claims:

- `kernel_trace.page.cursor` used symbolic synthetic cursors such as
  `trace:page:2` instead of numeric KMP `Trace.next_cursor` strings;
- stop scoring accepted the action type without requiring exact
  `answer_policy` and `final_refs`;
- model-facing `prepared_tool_call` rows were not fully validated against the
  same predictor contract and visible prepared payload used at inference time.

Use this document only to understand the experiment history. The clean
successor is
[`2026-05-17-writer-orchestration-v2-kmp-cursor.md`](2026-05-17-writer-orchestration-v2-kmp-cursor.md).

## 1. Purpose

Train a single Operator profile that can orchestrate the two writer phases that
were proven separately:

- writer pre-read: choose bounded `kernel_near`, `kernel_trace`,
  `kernel_inspect`, or stop;
- writer execution: choose a prepared write payload source for
  `kernel_write_memory` or `kernel_ingest`, or stop.

This is the first cut where Operator can decide whether the writer still needs
more memory context or whether the prepared write can be executed. The semantic
write payload is still prepared outside the Operator model. Operator does not
invent relations, text, evidence, or full write payloads.

## 2. Contract Change

The SFT preparer now supports:

```text
--capability-split-profile writer-orchestration
```

The profile exposes only:

```text
kernel_near
kernel_trace
kernel_inspect
kernel_write_memory
kernel_ingest
stop
```

It combines the required capabilities from `writer-pre-read` and `writer-exec`
without exposing unrelated KMP/MCP tools. Model-facing `allowed_tools` is also
projected per row:

- `write_context_read` rows expose only `kernel_near`, `kernel_trace`,
  `kernel_inspect`;
- `write` rows expose only `kernel_write_memory`, `kernel_ingest`;
- `stop` remains available through the action contract.

Successful write targets remain compact:

```json
{"action":{"type":"prepared_tool_call","tool":"kernel_write_memory","source":"draft_write.prepared_arguments"}}
```

```json
{"action":{"type":"prepared_tool_call","tool":"kernel_ingest","source":"canonical_payload"}}
```

The deterministic executor resolves those prepared decisions into final
KMP/MCP tool calls by copying the visible payload exactly.

## 3. Dataset Slice

Dataset:

```text
/tmp/kernel-operator-sft-writer-orchestration-v1-20260517
```

Sources:

```text
/tmp/kernel-operator-conformance-writer-pre-read-v4-20260516/trajectories.jsonl
/tmp/kernel-operator-conformance-writer-exec-v1-source-kind-agent-20260517/trajectories.jsonl
```

Preparation command:

```text
python scripts/operator/prepare_operator_sft_dataset.py \
  --trajectories /tmp/kernel-operator-conformance-writer-pre-read-v4-20260516/trajectories.jsonl \
  --trajectories /tmp/kernel-operator-conformance-writer-exec-v1-source-kind-agent-20260517/trajectories.jsonl \
  --output /tmp/kernel-operator-sft-writer-orchestration-v1-20260517 \
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

| Metric | Value |
| --- | ---: |
| rows | 810 |
| train rows | 567 |
| eval rows | 243 |
| `write_context_read` rows | 576 |
| `write` rows | 234 |
| capability coverage | 40 / 40 |
| train capability coverage | 40 / 40 |
| eval capability coverage | 40 / 40 |
| duplicate model-row hashes | 0 |
| train/eval model-row overlap | 0 |
| dropped non-visible target refs | 0 |
| dropped non-visible target cursors | 0 |

Model-facing action distribution:

| Action | Count |
| --- | ---: |
| `tool_call:kernel_trace` | 288 |
| `stop` | 270 |
| `prepared_tool_call` | 108 |
| `tool_call:kernel_near` | 72 |
| `tool_call:kernel_inspect` | 72 |

Eval split distribution:

| Mode | Rows |
| --- | ---: |
| `write_context_read` | 171 |
| `write` | 72 |

## 4. Oracle Gate

Before training, the model-facing eval was checked with the oracle baseline and
prepared-payload resolution enabled:

```text
cargo run -p underpass-operator-evaluation-cli --bin underpass_operator_policy_eval -- \
  --model-facing-eval /tmp/kernel-operator-sft-writer-orchestration-v1-20260517/eval.jsonl \
  --baseline oracle \
  --resolve-prepared-payloads \
  --output /tmp/kernel-operator-sft-writer-orchestration-v1-20260517-oracle-policy-eval.json \
  --details-output /tmp/kernel-operator-sft-writer-orchestration-v1-20260517-oracle-policy-details.jsonl
```

Result:

| Metric | Value |
| --- | ---: |
| exact action | 243 / 243 |
| invalid predictions | 0 |
| unbounded tool calls | 0 |
| `write_context_read` exact | 171 / 171 |
| `write` exact | 72 / 72 |

Resolved eval final-action distribution:

| Final action | Target |
| --- | ---: |
| `kernel_trace` | 91 |
| `stop` | 76 |
| `kernel_near` | 24 |
| `kernel_write_memory` | 24 |
| `kernel_inspect` | 20 |
| `kernel_ingest` | 8 |

## 5. Model And Recipe

| Field | Value |
| --- | --- |
| base model | `Qwen/Qwen2.5-0.5B-Instruct` |
| method | LoRA SFT |
| GPUs | 4 x RTX 3090 |
| epochs | 5 |
| batch size | 4 |
| grad accumulation | 1 |
| max length | 3072 |
| prediction batch size | 4 |
| prediction max new tokens | 256 |
| prediction temperature | 0.0 |
| prediction resolver | `--resolve-prepared-payloads` |
| schema mode | strict JSON, no additional properties |

Training completed cleanly.

| Metric | Value |
| --- | ---: |
| train runtime | 499.5 s |
| final train loss | 0.2205 |
| final eval loss | 0.01716 |
| final eval token accuracy | 0.9945 |
| prediction runtime | 3m27s |
| prediction failures | 0 / 243 |

Kubernetes training job:

```text
kop-qwen05-lora-worch-v1-4gpu-20260517
```

Kubernetes prediction job:

```text
kop-qwen05-predict-worch-v1-20260517
```

## 6. Strict Resolved Policy Eval

Evaluation used:

```text
underpass_operator_policy_eval \
  --model-facing-eval /tmp/kernel-operator-sft-writer-orchestration-v1-20260517/eval.jsonl \
  --predictions /tmp/kernel-operator-qwen05-predictions-writer-orchestration-v1-20260517/predictions.jsonl \
  --resolve-prepared-payloads
```

Result:

| Metric | Value |
| --- | ---: |
| exact action | 243 / 243, 100% |
| action type | 243 / 243, 100% |
| tool | 167 / 167, 100% |
| primary refs | 167 / 167, 100% |
| scope/about | 167 / 167, 100% |
| stop | 76 / 76, 100% |
| trace continuation | 48 / 48, 100% |
| cursor mode | 24 / 24, 100% |
| window shape | 24 / 24, 100% |
| limit policy | 24 / 24, 100% |
| invalid predictions | 0 |
| unbounded tool calls | 0 |

By mode:

| Mode | Exact | Notes |
| --- | ---: | --- |
| `write_context_read` | 171 / 171 | Read navigation before writing. |
| `write` | 72 / 72 | Prepared write execution and fail-fast stops. |

Target and predicted final-action distributions matched exactly:

| Final action | Target | Predicted |
| --- | ---: | ---: |
| `stop` | 76 | 76 |
| `kernel_trace` | 91 | 91 |
| `kernel_near` | 24 | 24 |
| `kernel_inspect` | 20 | 20 |
| `kernel_write_memory` | 24 | 24 |
| `kernel_ingest` | 8 | 8 |

## 7. Raw De-Anonymized Eval

Predictions were de-anonymized back to raw dataset refs:

```text
python scripts/operator/deanonymize_operator_predictions.py \
  --raw-trajectories /tmp/kernel-operator-sft-writer-orchestration-v1-20260517/eval_trajectories.jsonl \
  --model-trajectories /tmp/kernel-operator-sft-writer-orchestration-v1-20260517/eval_model_trajectories.jsonl \
  --predictions /tmp/kernel-operator-qwen05-predictions-writer-orchestration-v1-20260517/predictions.jsonl \
  --output /tmp/kernel-operator-qwen05-predictions-writer-orchestration-v1-20260517-raw-resolved \
  --force
```

Result:

| Metric | Value |
| --- | ---: |
| selected rows | 243 |
| written predictions | 243 |
| failures | 0 |
| mapped about values | 248 |
| mapped ref values | 1,449 |

Raw policy eval against `eval_trajectories.jsonl` also reached 243 / 243 exact
actions with 0 invalid predictions and 0 unbounded tool calls.

## 8. Decision

This run is quarantined. It passed the older offline mixed
writer-orchestration gate, but it does not pass the later KMP-cursor and
model-facing validation standard.

The product result is important:

- Operator can choose bounded pre-read actions and prepared write execution in
  one profile;
- write semantics still stay outside the 0.5B model;
- full write payload copy fidelity is handled by deterministic prepared-payload
  resolution;
- the model-facing tool surface no longer leaks unrelated KMP/MCP tools into
  substeps.

Live replay is intentionally not claimed here. The mixed eval contains synthetic
read refs from conformance fixtures; live replay requires either seeded
synthetic memory or a real benchmark trace where the refs exist in the deployed
kernel.

## 9. Why This Is Not Promoted

The old promotion gate for this run was:

- strict JSON parsing: 0 invalid predictions;
- exact resolved model-facing policy eval over all 243 eval rows;
- no unbounded tool calls;
- no tool leakage outside model-facing `allowed_tools`;
- exact stop behavior for both read and write modes;
- exact prepared-payload source selection for both write tools.

These gates passed under the older validator only. They are no longer
sufficient for promotion because they did not enforce real KMP trace cursor
shape, exact stop evidence, and full prepared-payload contract validation.

This run stays quarantined even though the historical numbers are preserved.

If it fails, keep the run and classify failures by mode:

- wrong read tool selection;
- wrong read bounds/cursor/page decision;
- premature stop before write context is sufficient;
- write execution when fail-fast stop was required;
- stop when prepared write execution was valid;
- invalid JSON or additional properties;
- prepared payload resolution failure.

## 10. Artifact Paths

| Artifact | Path |
| --- | --- |
| SFT dataset | `/tmp/kernel-operator-sft-writer-orchestration-v1-20260517` |
| oracle policy eval | `/tmp/kernel-operator-sft-writer-orchestration-v1-20260517-oracle-policy-eval.json` |
| oracle policy details | `/tmp/kernel-operator-sft-writer-orchestration-v1-20260517-oracle-policy-details.jsonl` |
| LoRA adapter | `/tmp/kernel-operator-qwen05-lora-writer-orchestration-v1-4gpu-20260517` |
| predictions | `/tmp/kernel-operator-qwen05-predictions-writer-orchestration-v1-20260517` |
| model-facing policy eval | `/tmp/kernel-operator-qwen05-predictions-writer-orchestration-v1-20260517-model-facing-policy-eval.json` |
| model-facing policy details | `/tmp/kernel-operator-qwen05-predictions-writer-orchestration-v1-20260517-model-facing-policy-details.jsonl` |
| raw predictions | `/tmp/kernel-operator-qwen05-predictions-writer-orchestration-v1-20260517-raw-resolved` |
| raw policy eval | `/tmp/kernel-operator-qwen05-predictions-writer-orchestration-v1-20260517-raw-policy-eval.json` |

Kubernetes jobs:

| Job | Status |
| --- | --- |
| `kop-qwen05-lora-worch-v1-4gpu-20260517` | Complete |
| `kop-qwen05-predict-worch-v1-20260517` | Complete |

## 11. Artifact Hashes

| Artifact | SHA-256 |
| --- | --- |
| SFT summary | `c6a26a9e8b83c20e71e9d5e974ec346129664e03a09c7b3be62b45dd9091b5cf` |
| oracle policy eval | `38c8069238a72a9d0a935672a1fb1becea24e183d69d49fe0f9ec8fbde2bd619` |
| prediction summary | `f8fd8e211f019b7f03eabbbf01d0de6eb9db1be6cd957eeb91fa3cc950b94f34` |
| model-facing policy eval | `5d1b5fc369d4575ebdfbff704212a20d42484f2b9f5e971ad97d18232fa2382d` |
| raw de-anonymized summary | `b89b4dc5940211c9e8f830d8ad7b219b14c57248d23bf4ef85df4df295ae0b9f` |
| raw policy eval | `823e95b5dba941b6b487dee1224803e5be459d7a15e72f9eb9471f481b478a0d` |
