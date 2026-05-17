# 2026-05-17 Writer Exec V1 Qwen 0.5B Copy-Fidelity Run

Status: failed
Date: 2026-05-17

## 1. Purpose

Train a separate Qwen 0.5B LoRA for the `writer-exec` profile.

This profile is intentionally narrower than writer pre-read. It does not read
memory, navigate temporal paths, infer relations, or choose semantic meaning. It
only executes already-prepared write payloads or stops fail-fast when execution
would violate the KMP write contract.

The gate for this run was strict:

```text
70 / 70 exact model-facing write actions
0 invalid predictions
0 unbounded tool calls
```

## 2. Dataset Slice

Suite:

```text
writer-exec-v1
```

The dataset was generated before training and audited separately.

| Metric | Value |
| --- | ---: |
| trajectories | 234 |
| train rows | 164 |
| eval rows | 70 |
| unique model-facing rows | 234 |
| duplicate model-row hashes | 0 |
| train/eval model-row overlap | 0 |
| writer-exec capability coverage | 20 / 20 |
| no-gold audit findings | 0 |
| oracle model-facing policy eval | 70 / 70 exact |

Target action distribution:

| Action | Count |
| --- | ---: |
| `stop` | 126 |
| `kernel_write_memory` | 72 |
| `kernel_ingest` | 36 |

Eval action distribution:

| Action | Count |
| --- | ---: |
| `stop` | 40 |
| `kernel_write_memory` | 23 |
| `kernel_ingest` | 7 |

## 3. Model And Recipe

| Field | Value |
| --- | --- |
| base model | `Qwen/Qwen2.5-0.5B-Instruct` |
| method | LoRA SFT |
| GPUs | 4 x RTX 3090 |
| epochs | 5 |
| batch size | 4 |
| grad accumulation | 1 |
| max length | 3072 |
| prediction batch size | 2 |
| prediction max new tokens | 1600 |
| prediction temperature | 0.0 |
| schema mode | strict JSON, no additional properties |

Training completed cleanly.

| Metric | Value |
| --- | ---: |
| train runtime | 163.4 s |
| final train loss | 0.4891 |
| final eval loss | 0.09361 |
| final eval token accuracy | 0.9808 |
| prediction failures | 0 / 70 |

## 4. Strict Model-Facing Policy Eval

Evaluation used the anonymized model-facing eval file:

```text
underpass_operator_policy_eval --model-facing-eval /tmp/kernel-operator-sft-writer-exec-v1-20260516/eval.jsonl
```

Result:

| Metric | Value |
| --- | ---: |
| exact action | 69 / 70, 98.57% |
| action type | 70 / 70, 100% |
| tool | 30 / 30, 100% |
| primary refs | 30 / 30, 100% |
| scope/about | 30 / 30, 100% |
| stop | 40 / 40, 100% |
| invalid predictions | 0 |
| unbounded tool calls | 0 |

Target and predicted action distributions matched exactly:

| Action | Target | Predicted |
| --- | ---: | ---: |
| `stop` | 40 | 40 |
| `kernel_write_memory` | 23 | 23 |
| `kernel_ingest` | 7 | 7 |

## 5. Failure Analysis

The single failure was not a routing failure. The model selected the correct
tool and produced a contract-valid `kernel_write_memory` call.

Failure step:

```text
kmp-operator-writer-exec-v1-20260516:writer-exec-v1-ci-flake-write-anemic-follows
```

The mismatch was inside `connect_to[0].why`.

Expected:

```text
The note follows the prior CI flaky test isolation decision in process order.
```

Predicted:

```text
The note follows the CI flaky test isolation decision in process order.
```

The model dropped the word `prior`.

This is semantically harmless but contractually important. `writer-exec` is a
prepared-payload execution profile. When the rule says "copy prepared arguments
exactly", the model must not rewrite even a small phrase inside the payload.

## 6. Decision

This run is not promotable.

It proves that the 0.5B model can learn the high-level writer-exec policy:

- choose `kernel_write_memory` versus `kernel_ingest` versus `stop`;
- reject unsafe writes;
- preserve tool, scope, and primary refs;
- emit strict valid JSON.

It does not yet prove byte-exact payload copying for long prepared writes.

The next design decision should be explicit:

1. Keep training the model to copy full prepared payloads exactly, with more
   copy-fidelity rows and stricter hard negatives.
2. Or change the runtime contract so Operator chooses a prepared write handle
   and a deterministic executor copies the already-validated payload into the
   KMP/MCP API call.

The second option is architecturally attractive because the model remains a
decision operator, while byte-exact copying stays deterministic. The final KMP
call can still be the real `kernel_write_memory` or `kernel_ingest` API call.

Follow-up implemented on 2026-05-17: `writer-exec` now trains on compact
`prepared_tool_call` decisions. The predictor and policy evaluator can resolve
those decisions through the deterministic prepared-payload executor, which
copies the visible payload into the final KMP/MCP action and validates it
against the action contract.

## 7. Artifact Paths

| Artifact | Path |
| --- | --- |
| conformance export | `/tmp/kernel-operator-conformance-writer-exec-v1-20260516` |
| SFT dataset | `/tmp/kernel-operator-sft-writer-exec-v1-20260516` |
| LoRA adapter | `/tmp/kernel-operator-qwen05-lora-writer-exec-v1-4gpu-20260517` |
| predictions | `/tmp/kernel-operator-qwen05-predictions-writer-exec-v1-20260517` |
| model-facing policy eval | `/tmp/kernel-operator-qwen05-predictions-writer-exec-v1-20260517-model-facing-policy-eval.json` |
| policy details | `/tmp/kernel-operator-qwen05-predictions-writer-exec-v1-20260517-model-facing-policy-details.jsonl` |

Kubernetes jobs:

| Job | Status |
| --- | --- |
| `kop-qwen05-lora-wexec-v1-4gpu-20260517` | Complete |
| `kop-qwen05-predict-wexec-v1-20260517` | Complete |

## 8. Artifact Hashes

| Artifact | SHA-256 |
| --- | --- |
| SFT summary | `3cf26e03aab9a2c51da040955069b28438dc177142a47d2df12a0af1cf575db3` |
| no-gold audit | `de9cc4147ad7e426dace39ede642035a323f758c2f73a0f83579f881aaf0b643` |
| oracle policy eval | `d0eaca6af4fd00ba1bf0ffd10a42a372640996ef6efb4767d35c9502d967f18e` |
| predictions | `3667c236bb68ede4ca037bc50449dcc310560e106f58eeb104e572f1c0d3ffba` |
| prediction summary | `daebddc1558111330378c7df365e4c44eb8e784ee4b15e7f009db943d5ca26c7` |
| model-facing policy eval | `580c4e8f6e193cdf41c039cb5e93307d41cc650d34aebf35fae0467002cd2294` |
