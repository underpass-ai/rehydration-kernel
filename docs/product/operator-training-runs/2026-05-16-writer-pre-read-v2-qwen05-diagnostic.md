# Operator Training Run: `writer-pre-read-v2-qwen05-diagnostic`

Status: `failed`

Date opened: 2026-05-16
Date closed: 2026-05-16
Owner: Tirso / Codex

## 1. Scope

| Field | Value |
| --- | --- |
| Attempt id | `writer-pre-read-v2-qwen05-diagnostic` |
| Profile | `writer-pre-read` |
| Base model | `Qwen/Qwen2.5-0.5B-Instruct` |
| Adapter output | `/tmp/kernel-operator-qwen05-lora-wpr-v2-4gpu-20260516` |
| Prediction output | `/tmp/kernel-operator-qwen05-predictions-wpr-v2-20260516` |
| Artifact root | `../rehydration-kernel-artifacts/operator/2026-05-16-writer-pre-read-v2-qwen05-diagnostic/` |
| Branch | `codex/operator-writer-pre-read-v2-train` |
| Commit | `a3adaad` |
| Dirty worktree at start | no |

## 2. North Star Check

```text
Operator 0.5B:
  only learns to use KMP.

Strong teacher:
  produces semantics when semantics are needed.

Kernel:
  validates, stores, traverses, proves, and audits memory.
```

This run respects the boundary because:

- the model is trained only to choose the next bounded KMP/MCP read move before
  a writer commits memory;
- it is not trained to author relation semantics, `why`, evidence, or final
  memory payloads;
- no teacher model is involved in this diagnostic run;
- kernel core remains deterministic and unchanged.

## 3. Hypothesis

Main hypothesis:

```text
The expanded writer-pre-read-v2 dataset is enough for Qwen 0.5B to learn the
read-before-write control policy, including near/inspect/trace/stop decisions,
trace continuation, sufficient-context stop, and ambiguous candidate handling.
```

Success means:

- strict prediction writes every eval action as valid
  `kernel-operator-action-contract-v1`;
- policy eval has zero invalid predictions;
- policy eval has zero unbounded tool calls;
- exact action accuracy is materially better than the previous diagnostic
  writer-pre-read result, especially for `kernel_trace` and `stop`;
- no capability in the `writer-pre-read` profile regresses silently.

Failure means:

- the model cannot emit strict JSON actions for this profile;
- the model maps `kernel_trace` or `stop` cases back to broad `kernel_near`;
- any prediction is unbounded;
- the result looks good only because the dataset has many duplicate
  model-facing rows.

## 4. Dataset Inputs

| Dataset | Source | Label source | Teacher model | Rows | Train | Eval | Status |
| --- | --- | --- | --- | ---: | ---: | ---: | --- |
| `p111-smart-writer-pre-read-mixed-v2-no-overlap-r05` | `/tmp/kernel-operator-sft-p111-smart-writer-pre-read-mixed-v2-no-overlap-r05-20260516` | benchmark-derived + deterministic conformance | none | 173 | 88 | 85 | audited |

## 5. Dataset Generation Commands

The dataset was generated in
[`2026-05-16-writer-pre-read-v2-diversity-gate.md`](2026-05-16-writer-pre-read-v2-diversity-gate.md).

Initial SFT source, rejected for training:

```text
/tmp/kernel-operator-sft-p111-smart-writer-pre-read-mixed-v2-compact-20260516/
```

Reason: it had 10 identical model-facing row hashes in both train and eval.

Corrected SFT source:

```bash
python scripts/operator/prepare_operator_sft_dataset.py \
  --trajectories /tmp/kernel-operator-trajectories-p111-smart-writer-pre-read-20260516/trajectories.jsonl \
  --trajectories /tmp/kernel-operator-conformance-writer-pre-read-v2-20260516/trajectories.jsonl \
  --output /tmp/kernel-operator-sft-p111-smart-writer-pre-read-mixed-v2-no-overlap-r05-20260516 \
  --include-mode write_context_read \
  --eval-ratio 0.5 \
  --split-mode group \
  --group-key task_or_step \
  --seed 42 \
  --anonymize-refs \
  --require-visible-target-refs \
  --require-visible-target-cursors \
  --capability-split-profile writer-pre-read \
  --require-eval-capability-coverage \
  --require-train-capability-coverage \
  --max-duplicate-model-row-count 20 \
  --drop-eval-model-row-overlap \
  --force
```

## 6. Dataset Evidence

| Evidence | Path / Value |
| --- | --- |
| SFT summary | `/tmp/kernel-operator-sft-p111-smart-writer-pre-read-mixed-v2-no-overlap-r05-20260516/summary.json` |
| debug audit | `/tmp/kernel-operator-sft-p111-smart-writer-pre-read-mixed-v2-no-overlap-r05-20260516/debug_audit.jsonl` |
| no-gold audit | `/tmp/kernel-operator-sft-p111-smart-writer-pre-read-mixed-v2-no-overlap-r05-20260516/no_gold_audit.json` |
| train rows | 88 |
| eval rows | 85 |
| unique model-facing rows | 24 |
| train unique model-facing rows | 15 |
| eval unique model-facing rows | 9 |
| train/eval model-facing overlap | 0 |
| train/eval full-row overlap | 0 |
| dropped eval rows due train overlap | 40 |
| dropped non-visible refs | 0 |
| dropped non-visible cursors | 0 |
| train contract coverage | 21 / 21, 100% |
| eval contract coverage | 21 / 21, 100% |

Decision after dataset audit:

```text
retroactive verdict: diagnostic-only, not trainable for a serious policy claim
reason: coverage is complete and train/eval overlap is zero, but unique prompt
diversity remains low and rare actions are underrepresented.
```

Retrospective dataset quality audit against
[`operator-dataset-quality-contract.md`](../operator-dataset-quality-contract.md):

| Check | Observed | Verdict |
| --- | ---: | --- |
| raw selected rows before quality filters | 6,136 | misleading as quality signal |
| selected rows after quality filters | 173 | usable for diagnostic only |
| unique model-facing rows | 24 | too low |
| duplicate model-row extra rows | 149 | too high |
| max duplicate model-row count | 20 | too high |
| train/eval model-row overlap | 0 | pass |
| train `kernel_trace` targets | 2 | too low |
| train `stop` targets | 1 | too low |
| eval `kernel_trace` targets | 2 | too low |
| eval `stop` targets | 1 | too low |
| train dominant action | `kernel_inspect` 58 / 88 | collapse risk |
| eval majority baseline | `kernel_inspect` 41 / 85 = 48.2% | must beat materially |
| synthetic use-case minimums | not met for trace/stop and contrastive boundaries | fail |

This dataset should have been stopped before GPU as `diagnostic-only`. It is
safe for a negative probe, but it is not a strong writer-pre-read training cut.

## 7. Training Configuration

| Field | Value |
| --- | --- |
| train jsonl | `/tmp/kernel-operator-sft-p111-smart-writer-pre-read-mixed-v2-no-overlap-r05-20260516/openai_train.jsonl` |
| eval jsonl | `/tmp/kernel-operator-sft-p111-smart-writer-pre-read-mixed-v2-no-overlap-r05-20260516/openai_eval.jsonl` |
| model id | `Qwen/Qwen2.5-0.5B-Instruct` |
| model revision | unknown |
| tokenizer revision | unknown |
| epochs | 8 |
| batch size | 4 |
| grad accumulation | 1 |
| max length | 3072 |
| dtype | fp16 |
| LoRA r | 16 |
| LoRA alpha | 32 |
| LoRA target modules | `q_proj,k_proj,v_proj,o_proj,gate_proj,up_proj,down_proj` |
| hardware | Kubernetes, 4 x RTX 3090 |
| job id | `kop-qwen05-lora-wpr-v2-4gpu-20260516` |

Command or manifest:

```text
k8s/kernel-operator-qwen05-lora-wpr-v2-4gpu-20260516-job.yaml
```

## 8. Live Training Journal

| Time | Event | Evidence | Decision |
| --- | --- | --- | --- |
| 20:43 CEST | Kubernetes training job launched | `kop-qwen05-lora-wpr-v2-4gpu-20260516` | continue |
| 20:49 CEST | Job deleted before completion to verify dataset first | `kubectl -n underpass-runtime delete job kop-qwen05-lora-wpr-v2-4gpu-20260516` | do not use partial output |
| 20:51 CEST | Initial compact dataset rejected for eval | 10 train/eval model-row hash overlaps | switch to no-overlap r05 |
| 20:53 CEST | Corrected no-overlap r05 dataset passed gates | no-gold 0, train/eval coverage 21/21, overlap 0 | continue |
| 20:54 CEST | Kubernetes training job relaunched with corrected dataset | `kop-qwen05-lora-wpr-v2-4gpu-20260516` | continue |

## 8.1 Capability And Data Contribution

| Data block | Intended capability | Added rows | Coverage delta | Strict metric delta | Classification |
| --- | --- | ---: | --- | --- | --- |
| `writer-pre-read-v2` conformance | trace continuation, stop, ambiguity | 14 | 16 -> 21 required capabilities | pending | unproven |
| P1.11 smart-writer pre-read rows | real benchmark writer pre-read behavior | 199 | keeps real near/inspect distribution | pending | unproven |

## 9. Stop Gates

| Gate | Required | Observed | Pass |
| --- | --- | --- | --- |
| correct dataset selected | yes | `/tmp/kernel-operator-sft-p111-smart-writer-pre-read-mixed-v2-no-overlap-r05-20260516` | yes |
| correct model selected | yes | `Qwen/Qwen2.5-0.5B-Instruct` | yes |
| no-gold audit findings | 0 | 0 | yes |
| dropped non-visible target refs | 0 | 0 | yes |
| dropped non-visible target cursors | 0 | 0 | yes |
| declared profile coverage in train | 100% | 100% | yes |
| declared profile coverage in eval | 100% | 100% | yes |
| dataset quality contract verdict | trainable for declared tier | diagnostic-only | no |
| majority-action collapse risk | low or accepted before training | high; `kernel_inspect` baseline 48.2% | no |
| rare actions have enough unique rows | yes | no; trace/stop underrepresented | no |
| synthetic use-case coverage | profile minimums met | no; v2 conformance is too small | no |
| prompt/tool parity | yes | partial; generic prompt lists shapes outside active `allowed_tools` | no |
| invalid predictions | 0 for candidate | 0 | yes |
| unbounded tool calls | 0 | 0 | yes |
| MCP replay failures | 0 | skipped after offline policy failure | no |
| missing expected refs | 0 | skipped after offline policy failure | no |
| cost/time budget exceeded | no | no | yes |

Pause/stop decisions:

```text
training should have been blocked by dataset quality contract.
run kept as failed diagnostic evidence.
```

## 10. Training Result

| Metric | Value |
| --- | ---: |
| final train loss | pending |
| final train loss | 0.4247 |
| final eval loss | 0.02203 |
| best eval loss | 0.02203 |
| epoch of best eval | 8 |
| runtime | 210.9s trainer runtime; 4m8s Kubernetes job duration |

Training interpretation:

```text
the model learned the surface format and repeated the dominant policy pattern.
low eval loss did not mean policy learning.
```

## 11. Prediction And Policy Eval

| Metric | Value |
| --- | ---: |
| eval rows | 85 |
| parsed predictions | 85 |
| prediction failures | 0 |
| invalid predictions | 0 |
| unbounded tool calls | 0 |
| exact action accuracy | 0.4823529412 |
| tool accuracy | 0.4880952381 |
| primary ref accuracy | 0.9642857143 |
| scope accuracy | 0.5119047619 |
| stop accuracy | 0.0 |

Failure shape:

| Target | Prediction | Count |
| --- | --- | ---: |
| `stop` | `kernel_inspect` | 1 |
| `kernel_inspect` | `kernel_inspect` | 41 |
| `kernel_near` | `kernel_inspect` | 41 |
| `kernel_trace` | `kernel_inspect` | 2 |

The model predicted `kernel_inspect` for all 85 eval rows.

Artifacts:

| Artifact | Path |
| --- | --- |
| predictions | `/tmp/kernel-operator-qwen05-predictions-wpr-v2-20260516/predictions.jsonl` |
| raw model results | `/tmp/kernel-operator-qwen05-predictions-wpr-v2-20260516/llm_results.jsonl` |
| policy eval | `/tmp/kernel-operator-qwen05-policy-eval-wpr-v2-20260516.json` |
| policy details | `/tmp/kernel-operator-qwen05-policy-details-wpr-v2-20260516.jsonl` |

Copied artifacts:

```text
../rehydration-kernel-artifacts/operator/2026-05-16-writer-pre-read-v2-qwen05-diagnostic/results/
```

Artifact hashes:

| Artifact | SHA-256 |
| --- | --- |
| dataset `summary.json` | `33d777adad6886ea7762e970409bbc7792e51387a56302f4e5cfa2f86b919dcd` |
| dataset `no_gold_audit.json` | `f5d95e46297dcd1ecbf2ae5782bc748ed38681841fa5954cceb3b42ced583221` |
| dataset `openai_train.jsonl` | `ae5922d63fb99e99cbac9bef7dc0d7b56b3d84cdb4bdd2c2ad8e9e91fb3fc364` |
| dataset `openai_eval.jsonl` | `28e718bda96002b55bc67eef65301236cab027769ae79a09821636db07050cd3` |
| adapter `adapter_model.safetensors` | `41850f5da08df979fdce8fad4abb4e3a7b2f793f6032d4db76fb5970d5eeebe2` |
| predictions `summary.json` | `a8faa11df3dadfe544fbc583cc77633993e924aa07dbd1db4eaebba1e08cc622` |
| policy eval | `04736edd55d0d1fa7642203ba944916dbf3ac1c442d8288694f2ff4ec1e24a7c` |
| policy details | `5a8e91220e0b7ca228f11648ccb59175149cdea30b290961e7972a829389d5b9` |

## 12. Baseline Comparison

| Baseline | Metric | Baseline | This run | Delta |
| --- | --- | ---: | ---: | ---: |
| `read-rare-v1` writer rows | exact `write_context_read` actions | 4 / 8 | pending | pending |

Classification:

```text
failed diagnostic.
no promotion, no replay claim, no publication claim.
```

## 13. De-Anonymized Eval And MCP Replay

This is a diagnostic writer pre-read training run. Live MCP replay is valuable
only if offline policy evaluation is clean enough to justify the extra step.

| Check | Value |
| --- | ---: |
| de-anonymized predictions | pending |
| raw policy exact accuracy | pending |
| replay limit smoke | pending |
| replay full rows | pending |
| MCP tool successes | pending |
| MCP failures | pending |

Skipped because offline policy evaluation failed.

## 14. Required Next Dataset Cut

Create `writer-pre-read-v3` before training again.

Requirements:

- use the Operator dataset quality contract before launching GPU;
- reduce duplicate model rows instead of relying on copied rows;
- create a balanced training cut and a natural distribution eval cut;
- add contrastive families for `near` versus `inspect`, `inspect` versus
  `stop`, and `trace first` versus `trace continue`;
- generate enough synthetic rows per writer-pre-read use case before mixing
  real MemoryArena traces;
- increase unique examples for `kernel_trace` and `stop`;
- record majority-action baseline before training;
- use a profile-specific prompt that lists only tools available for
  `writer-pre-read`;
- mark observed benchmark trajectories as observed signal unless audited by a
  deterministic policy, strong teacher, or human review.
