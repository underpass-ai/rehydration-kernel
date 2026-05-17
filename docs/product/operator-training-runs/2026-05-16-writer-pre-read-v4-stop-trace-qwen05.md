# 2026-05-16 Writer Pre-Read V4 Stop/Trace Qwen 0.5B Run

Status: quarantined
Date: 2026-05-16

2026-05-17 follow-up: this original standalone run is quarantined under the
current strict contract because its SFT rows used symbolic trace continuation
cursors. The replacement standalone dataset is:

```text
../rehydration-kernel-artifacts/operator/2026-05-17-kmp-cursor-contract-refresh/kernel-operator-sft-writer-pre-read-v4-kmp-cursor-v2-20260517
```

The replacement keeps the 576-row writer-pre-read surface, passes trainer and
predictor validate-only preflights, and reports 21/21 writer-pre-read target
capability coverage with numeric KMP trace cursors. Use the preserved artifact
root above for current training; `/tmp` was only the regeneration scratch area.

## 1. Purpose

Close the only remaining writer-pre-read v3b failure:

```text
expected stop, predicted kernel_trace
```

The failure was not a JSON problem. It was a policy boundary problem. The model
had learned to trace when relation endpoints were visible, but it still needed
stronger contrastive examples for the case where tracing is already complete,
evidence is sufficient, and no tool budget remains.

## 2. Dataset Slice

Suite:

```text
writer-pre-read-v4
```

V4 keeps the cohesive v3 writer-pre-read surface and adds focused hard negatives
around the `stop` versus `kernel_trace` boundary.

The added v4 families are:

| Family | Correct decision |
| --- | --- |
| `trace_needed_after_inspect` | call `kernel_trace` because endpoints are visible but the path has not been read |
| `trace_continue_when_page_has_more` | call `kernel_trace` with continuation because the previous trace page is partial |
| `stop_after_complete_trace_zero_budget` | stop because the trace is complete, evidence is sufficient, and no tool calls remain |

Export summary:

| Metric | Value |
| --- | ---: |
| trajectories | 576 |
| v3 base rows | 360 |
| v4 hard-negative rows | 216 |
| target `kernel_trace` | 288 |
| target `stop` | 144 |
| target `kernel_near` | 72 |
| target `kernel_inspect` | 72 |
| contract validation failures | 0 |

Prepared SFT summary:

| Metric | Value |
| --- | ---: |
| train rows | 403 |
| eval rows | 173 |
| unique model-facing rows | 576 |
| duplicate row hashes | 0 |
| train/eval model-row overlap | 0 |
| writer profile capability coverage | 21 / 21 |
| no-gold audit findings | 0 |

The model-facing prompt includes visible state fields derived only from the
decision-time state:

- `remaining_tool_calls`;
- `last_result_has_more`;
- `last_result_partial`;
- `trace_complete`.

These fields do not expose the target action. They make the stop/trace boundary
legible to a 0.5B model without hiding policy in nested JSON.

## 3. Model And Recipe

| Field | Value |
| --- | --- |
| base model | `Qwen/Qwen2.5-0.5B-Instruct` |
| method | LoRA SFT |
| GPUs | 4 x RTX 3090 |
| epochs | 5 |
| batch size | 4 |
| max length | 3072 |
| prediction temperature | 0.0 |
| schema mode | strict JSON, no additional properties |

Training finished cleanly:

| Metric | Value |
| --- | ---: |
| train runtime | 265.1 s |
| final eval loss | 0.01267 |
| final eval token accuracy | 0.9952 |
| prediction failures | 0 / 173 |

## 4. Strict Model-Facing Policy Eval

Evaluation used:

```text
underpass_operator_policy_eval --model-facing-eval /tmp/kernel-operator-sft-writer-pre-read-v4-20260516/eval.jsonl
```

This is the correct target namespace for anonymized SFT prompts. Raw trajectory
refs were not mixed with model-facing refs.

| Metric | Value |
| --- | ---: |
| exact action | 173 / 173, 100% |
| action type | 100% |
| tool | 100% |
| primary refs | 100% |
| scope/about | 100% |
| cursor mode | 100% |
| window | 100% |
| limit | 100% |
| trace page continuation | 47 / 47, 100% |
| stop | 35 / 35, 100% |
| invalid predictions | 0 |
| unbounded calls | 0 |

Target and predicted action distributions matched exactly:

| Action | Target | Predicted |
| --- | ---: | ---: |
| `stop` | 35 | 35 |
| `kernel_inspect` | 20 | 20 |
| `kernel_near` | 28 | 28 |
| `kernel_trace` | 90 | 90 |

## 5. Decision

The v4 cohesive writer-pre-read slice fixes the known v3b stop/trace gap under
strict offline model-facing evaluation.

This should be treated as a passed writer-pre-read profile cut, not yet as a
promoted Operator model. The next promotion gates are:

- broader natural writer traces;
- live MCP/KMP replay for the writer pre-read profile;
- mixed read plus writer routing without exposing the wrong tool surface;
- a prepared-write dataset that is separate from pre-read decisions.

## 6. Artifact Paths

| Artifact | Path |
| --- | --- |
| conformance export | `/tmp/kernel-operator-conformance-writer-pre-read-v4-20260516` |
| SFT dataset | `/tmp/kernel-operator-sft-writer-pre-read-v4-20260516` |
| LoRA adapter | `/tmp/kernel-operator-qwen05-lora-wpr-v4-4gpu-20260516` |
| predictions | `/tmp/kernel-operator-qwen05-predictions-wpr-v4-20260516` |
| model-facing policy eval | `/tmp/kernel-operator-qwen05-predictions-wpr-v4-20260516-model-facing-policy-eval.json` |
| policy details | `/tmp/kernel-operator-qwen05-predictions-wpr-v4-20260516-model-facing-policy-details.jsonl` |

## 7. Artifact Hashes

| Artifact | SHA-256 |
| --- | --- |
| conformance summary | `3e9445194cba765f6a8d0219631d0ac27b6b5c55fea9e373198fef221a51f46a` |
| SFT summary | `51cad72a2e726c07d47aca8c31aab76793335b29ec827efa4b4f97aab395bc9f` |
| no-gold audit | `fb5a1137af729a6ec16c65e3d3cf84110d14f2256a9df393d61b9a4715372165` |
| predictions | `c6ee915c7d6825186af765a0ed23dca88310e0788c611b6871091815d5fbba21` |
| model-facing policy eval | `55870b586dd0ee178e2b97461b5e7a0616b76068c4319d7bf6e184c2d429e231` |
