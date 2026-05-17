# 2026-05-17 Writer Exec Prepared V1 Qwen 0.5B Run

Status: writer-exec-profile-passed
Date: 2026-05-17

## 1. Purpose

Train Qwen 0.5B on the new `writer-exec` boundary where the model chooses a
prepared payload source and a deterministic executor copies that payload into
the final KMP/MCP write call.

This replaces the previous copy-fidelity objective. The model no longer emits
full `kernel_write_memory` or `kernel_ingest` payloads.

## 2. Dataset Slice

Dataset:

```text
/tmp/kernel-operator-sft-writer-exec-v1-prepared-exec-20260517
```

Follow-up corrected dataset for live replay:

```text
../rehydration-kernel-artifacts/operator/2026-05-17-kmp-cursor-contract-refresh/kernel-operator-sft-writer-exec-v1-prepared-exec-source-kind-agent-20260517
```

2026-05-17 follow-up audit: the original standalone Kubernetes train/predict
manifests are now quarantined. The strict preflight correctly rejects the
original SFT rows because resolving prepared payloads exposes
`source_kind=synthetic_conformance`. The valid standalone path is the
`source-kind-agent` dataset.

The corrected dataset keeps the same 164/70 split and changes only the
synthetic `source_kind` from the invalid value `synthetic_conformance` to the
canonical KMP value `agent`. Evidence `source` strings still preserve the
synthetic provenance label.

| Metric | Value |
| --- | ---: |
| rows | 234 |
| train rows | 164 |
| eval rows | 70 |
| model-facing `prepared_tool_call` rows | 108 |
| model-facing `stop` rows | 126 |
| writer-exec capability coverage | 20 / 20 |
| train/eval model-row overlap | 0 |
| duplicate model-row hashes | 0 |
| no-gold audit findings | 0 |
| resolved oracle policy eval | 70 / 70 exact |

Eval final-action distribution after deterministic resolution:

| Final action | Target |
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
| prediction batch size | 4 |
| prediction max new tokens | 256 |
| prediction temperature | 0.0 |
| prediction resolver | `--resolve-prepared-payloads` |
| schema mode | strict JSON, no additional properties |

Training completed cleanly.

| Metric | Value |
| --- | ---: |
| train runtime | 106.1 s |
| final train loss | 0.6292 |
| final eval loss | 0.1113 |
| final eval token accuracy | 0.9760 |
| prediction failures | 0 / 70 |

## 4. Strict Resolved Policy Eval

Evaluation used:

```text
underpass_operator_policy_eval \
  --model-facing-eval /tmp/kernel-operator-sft-writer-exec-v1-prepared-exec-20260517/eval.jsonl \
  --predictions /tmp/kernel-operator-qwen05-predictions-writer-exec-prepared-v1-20260517/predictions.jsonl \
  --resolve-prepared-payloads
```

Result:

| Metric | Value |
| --- | ---: |
| exact action | 70 / 70, 100% |
| action type | 70 / 70, 100% |
| tool | 30 / 30, 100% |
| primary refs | 30 / 30, 100% |
| scope/about | 30 / 30, 100% |
| stop | 40 / 40, 100% |
| invalid predictions | 0 |
| unbounded tool calls | 0 |

Target and predicted final-action distributions matched exactly:

| Final action | Target | Predicted |
| --- | ---: | ---: |
| `stop` | 40 | 40 |
| `kernel_write_memory` | 23 | 23 |
| `kernel_ingest` | 7 | 7 |

## 5. Live Replay And Source Kind Correction

The first live replay against
`https://rehydration-kernel.underpassai.com` exposed a real dataset/API gap:

```text
invalid memory provenance source_kind `synthetic_conformance`
```

This affected only the 7 `kernel_ingest` rows. The 23 `kernel_write_memory`
rows and 40 `stop` rows were correct. The root cause was the synthetic
conformance exporter using a non-canonical provenance enum value. KMP accepts
only:

```text
human | agent | projection | derived
```

Fix applied:

- `writer-exec-v1` synthetic payloads now use `source_kind: agent`;
- the dataset was regenerated with the same run id and `--eval-ratio 0.3`;
- prediction was rerun with the existing LoRA adapter and
  `--resolve-prepared-payloads`;
- strict resolved policy eval still reached 70 / 70 exact actions;
- raw de-anonymization still mapped 70 / 70 predictions with 0 failures;
- live MCP replay passed all 70 rows.

Corrected live replay result:

| Metric | Value |
| --- | ---: |
| selected rows | 70 |
| stop actions | 40 |
| executed tool calls | 30 |
| successful tool calls | 30 |
| failed tool calls | 0 |
| `kernel_write_memory` | 23 |
| `kernel_ingest` | 7 |
| missing predictions | 0 |
| invalid predictions | 0 |
| unbounded tool calls | 0 |
| missing expected ref rows | 0 |

Because the writer-exec eval uses `dry_run=true`, this replay validates the MCP
contract, prepared-payload execution, boundedness, fail-fast behavior, and
canonical KMP payload validation without mutating memory.

A separate live gRPC smoke was then executed with `kernel_ingest dry_run=false`
and a follow-up `kernel_inspect` over the committed entry:

| Metric | Value |
| --- | ---: |
| selected rows | 2 |
| executed tool calls | 2 |
| successful tool calls | 2 |
| failed tool calls | 0 |
| ingest commit latency | 1,699 ms |
| inspect latency | 539 ms |

This proves the deployed public TLS endpoint is not only accepting local MCP
dry-run validation; it can commit typed memory through gRPC and read it back.

## 6. Decision

This run passes the strict offline writer-exec profile gate and the live
KMP/MCP replay gate after the source-kind correction.

The important product result is not just the score. The architecture is now
cleaner:

- the writer or teacher still owns semantic payload construction;
- Operator decides whether a prepared write is executable;
- the deterministic executor performs byte-exact payload copying;
- KMP/MCP receives a normal validated `kernel_write_memory` or `kernel_ingest`
  call.

Promotion still requires mixed orchestration with the writer pre-read profile.

## 7. Artifact Paths

| Artifact | Path |
| --- | --- |
| SFT dataset | `/tmp/kernel-operator-sft-writer-exec-v1-prepared-exec-20260517` |
| corrected SFT dataset | `../rehydration-kernel-artifacts/operator/2026-05-17-kmp-cursor-contract-refresh/kernel-operator-sft-writer-exec-v1-prepared-exec-source-kind-agent-20260517` |
| LoRA adapter | `/tmp/kernel-operator-qwen05-lora-writer-exec-prepared-v1-4gpu-20260517` |
| corrected standalone LoRA retrain target | `../rehydration-kernel-artifacts/operator/runs/kernel-operator-qwen05-lora-writer-exec-prepared-source-kind-agent-4gpu-20260517` |
| predictions | `/tmp/kernel-operator-qwen05-predictions-writer-exec-prepared-v1-20260517` |
| corrected predictions | `../rehydration-kernel-artifacts/operator/runs/kernel-operator-qwen05-predictions-writer-exec-prepared-source-kind-agent-20260517` |
| model-facing policy eval | `/tmp/kernel-operator-qwen05-predictions-writer-exec-prepared-v1-20260517-model-facing-policy-eval.json` |
| corrected model-facing policy eval | `/tmp/kernel-operator-qwen05-predictions-writer-exec-prepared-source-kind-agent-20260517-model-facing-policy-eval.json` |
| policy details | `/tmp/kernel-operator-qwen05-predictions-writer-exec-prepared-v1-20260517-model-facing-policy-details.jsonl` |
| corrected raw predictions | `/tmp/kernel-operator-qwen05-predictions-writer-exec-prepared-source-kind-agent-20260517-raw-resolved` |
| corrected live replay | `/tmp/kernel-operator-qwen05-writer-exec-prepared-source-kind-agent-20260517-live-replay-full` |
| live gRPC commit smoke | `/tmp/kernel-operator-live-grpc-smoke-20260517` |

Kubernetes jobs:

| Job | Status |
| --- | --- |
| `kop-qwen05-lora-wexec-prep-v1-4gpu-20260517` | Quarantined, original invalid `source_kind` dataset |
| `kop-qwen05-predict-wexec-prep-v1-20260517` | Quarantined, original invalid `source_kind` dataset |
| `kop-qwen05-lora-wexec-prep-sk-agent-4gpu-20260517` | Current corrected standalone retrain manifest |
| `kop-qwen05-predict-wexec-prep-sk-agent-20260517` | Current corrected prediction manifest |

## 8. Artifact Hashes

| Artifact | SHA-256 |
| --- | --- |
| SFT summary | `09bf5a772be93bb4c339b5b5a67e272089c58de5a04f4520c98ccdaf03cbf404` |
| no-gold audit | `8508c8335467ac8c656c7c7ad980d1057ab2df11bd3937d23315d3673bd10655` |
| resolved oracle policy eval | `a169772e8b2b2cc5e68dfa40f030fe248a1fd1ddf71b37e27f8cfe3042272233` |
| predictions | `0c2f50e0aac9b2bea007a427d4907df33556721702f8b3195031f88378747281` |
| prediction summary | `990ab125b891997d5f4bc74fa0a6ad16c629452588bf7d679b533adcecc92d64` |
| model-facing policy eval | `5bfd93f2ee99c97ee36c4d4a2306ccf41d707a27e773607d1e81da1c4c249642` |
| corrected SFT summary | `6df2a058031d3d2ec1fb0f4ec88b5dfb25cc9d74265c452d7c1ac871281af498` |
| corrected prediction summary | `19bc477a535e0d1e1146b62ba3f9ade3c971e32eb3b8549e95dd834ea00faca1` |
| corrected model-facing policy eval | `d1f58c5136524869dd6fb7fd50be3f98449a53982b10c901c5b6c58e65c47b80` |
| corrected live replay summary | `129dab09d733324d780d8a873d0e4c3c8d9c094a66191df6b35a64f84eb800f9` |
| live gRPC smoke summary | `35adda2756d697d08b8eadc0380f95ec06229eed49ef91479addea54309fc166` |
