# Operator Training Run Template

Copy this file to:

```text
docs/product/operator-training-runs/YYYY-MM-DD-<attempt-id>-<scope>.md
```

Do not edit this template with run-specific data.

---

# Operator Training Run: `<attempt-id>`

Status: `planned | dataset-ready | running | paused | failed | quarantined | baseline-only | internal-only | promoted | aborted | read-profile-passed | writer-pre-read-profile-passed | writer-exec-profile-passed | writer-orchestration-profile-passed`

Date opened: `YYYY-MM-DD`
Date closed: `YYYY-MM-DD or pending`
Owner: `Tirso / Codex / other`

## 1. Scope

| Field | Value |
| --- | --- |
| Attempt id | `<attempt-id>` |
| Profile | `operator-read / writer-pre-read / writer-exec / writer-orchestration / writer-teacher / full` |
| Base model | `<model id>` |
| Adapter output | `<path or hub id>` |
| Artifact root | `../rehydration-kernel-artifacts/operator/<attempt-id>/` |
| Branch | `<branch>` |
| Commit | `<sha>` |
| Dirty worktree at start | `yes/no + summary` |

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

- `<explain why the Operator is not being trained as a semantic memory author>`;
- `<explain whether any teacher model is involved>`;
- `<explain whether kernel core remains deterministic>`.

## 3. Hypothesis

Main hypothesis:

```text
<What this run is expected to prove or disprove.>
```

Success means:

- `<metric/gate>`;
- `<metric/gate>`;
- `<metric/gate>`.

Failure means:

- `<specific failure condition>`;
- `<specific failure condition>`.

## 4. Dataset Inputs

| Dataset | Source | Label source | Teacher model | Rows | Train | Eval | Status |
| --- | --- | --- | --- | ---: | ---: | ---: | --- |
| `<name>` | `<path/command/run>` | `deterministic / benchmark-derived / gpt5_5_teacher / human` | `<none/model>` | `<n>` | `<n>` | `<n>` | `<planned/generated/audited>` |

## 5. Dataset Generation Commands

```bash
# Generate or export trajectories
<command>

# Prepare SFT rows
<command>

# Audit no-gold/no-leak
<command>

# Contract coverage
<command>
```

## 6. Dataset Evidence

| Evidence | Path / Value |
| --- | --- |
| trajectory summary | `<path>` |
| SFT summary | `<path>` |
| hashes | `<path or pasted values>` |
| debug audit | `<path>` |
| no-gold audit | `<path>` |
| dropped non-visible refs | `<n>` |
| dropped non-visible cursors | `<n>` |
| duplicate step ids | `<n>` |
| unique model-facing rows | `<n>` |
| unique train model-facing rows | `<n>` |
| unique eval model-facing rows | `<n>` |
| train/eval model-row overlap | `<n>` |
| full-row overlap | `<n>` |
| maximum duplicate model-row count | `<n>` |
| train action distribution | `<map>` |
| eval action distribution | `<map>` |
| unique rows per action, train | `<map>` |
| unique rows per action, eval | `<map>` |
| majority-action baseline | `<value>` |
| contrastive families | `<path/value>` |
| synthetic use-case families covered | `<n>/<n>` |
| synthetic use-case minimum counts | `<pass/fail + path/value>` |
| prompt/tool parity | `<yes/no>` |
| model-facing target projection | `<direct_kmp_action / prepared_payload_decision_v1>` |
| prepared-payload resolution required | `<yes/no>` |
| trace page cursor shape | `<numeric KMP Trace.next_cursor / not applicable / fail>` |
| stop evidence gate | `<answer_policy + final_refs exact / fail>` |
| source_kind validation | `<human|agent|projection|derived only / fail>` |
| contract validation failures | `<n>` |
| all contract coverage | `<value>` |
| train contract coverage | `<value>` |
| eval contract coverage | `<value>` |
| provenance audit | `<path/value>` |

Decision after dataset audit:

```text
continue / stop / quarantine
reason: <...>
```

## 7. Training Configuration

| Field | Value |
| --- | --- |
| train jsonl | `<path>` |
| eval jsonl | `<path>` |
| model id | `<model>` |
| model revision | `<revision or unknown>` |
| tokenizer revision | `<revision or unknown>` |
| epochs | `<n>` |
| batch size | `<n>` |
| grad accumulation | `<n>` |
| max length | `<n>` |
| dtype | `<bf16/fp16/fp32>` |
| LoRA r | `<n>` |
| LoRA alpha | `<n>` |
| LoRA target modules | `<modules>` |
| hardware | `<local/Kubernetes/GPU>` |
| job id | `<id>` |

Command or manifest:

```bash
<training command>
```

## 8. Live Training Journal

Add entries while the run is active.

| Time | Event | Evidence | Decision |
| --- | --- | --- | --- |
| `<HH:MM>` | `<started>` | `<log path>` | `<continue>` |

## 8.1 Capability And Data Contribution

Record whether each data block actually helped the declared profile.

| Data block | Intended capability | Added rows | Coverage delta | Strict metric delta | Classification |
| --- | --- | ---: | --- | --- | --- |
| `<name>` | `<capability>` | `<n>` | `<before -> after>` | `<metric delta>` | `improves / neutral / regresses / unsafe / unproven` |

Notes:

```text
<Explain why the data helped, did not help, or must be quarantined.>
```

## 9. Stop Gates

Stop immediately if any checked gate fails.

| Gate | Required | Observed | Pass |
| --- | --- | --- | --- |
| correct dataset selected | yes | `<value>` | `yes/no` |
| correct model selected | yes | `<value>` | `yes/no` |
| no-gold audit findings | 0 | `<n>` | `yes/no` |
| dropped non-visible target refs | 0 unless explicitly accepted | `<n>` | `yes/no` |
| declared profile coverage in train | 100% | `<value>` | `yes/no` |
| declared profile coverage in eval | 100% | `<value>` | `yes/no` |
| dataset quality contract verdict | trainable for declared tier | `<trainable/diagnostic-only/smoke-only/quarantine>` | `yes/no` |
| majority-action collapse risk | low or explicitly accepted | `<value>` | `yes/no` |
| rare actions have enough unique rows | yes | `<value>` | `yes/no` |
| synthetic use-case coverage | profile minimums met | `<value>` | `yes/no` |
| prompt/tool parity | yes | `<value>` | `yes/no` |
| invalid predictions | 0 for candidate | `<n>` | `yes/no` |
| unbounded tool calls | 0 | `<n>` | `yes/no` |
| MCP replay failures | 0 | `<n>` | `yes/no` |
| missing expected refs | 0 | `<n>` | `yes/no` |
| cost/time budget exceeded | no | `<value>` | `yes/no` |

Pause/stop decisions:

```text
<If the run was paused/stopped, record exact reason.>
```

## 10. Training Result

| Metric | Value |
| --- | ---: |
| final train loss | `<value>` |
| final eval loss | `<value>` |
| best eval loss | `<value>` |
| epoch of best eval | `<value>` |
| runtime | `<duration>` |

Training interpretation:

```text
<What happened during training. Mention deterioration, instability, or clean
convergence. Do not treat low loss as success unless strict policy eval and MCP
replay also pass.>
```

## 11. Prediction And Policy Eval

| Metric | Value |
| --- | ---: |
| eval rows | `<n>` |
| parsed predictions | `<n>` |
| prediction failures | `<n>` |
| invalid predictions | `<n>` |
| unbounded tool calls | `<n>` |
| exact action accuracy | `<value>` |
| tool accuracy | `<value>` |
| primary ref accuracy | `<value>` |
| scope accuracy | `<value>` |
| stop accuracy | `<value>` |

Failure reasons:

| Reason | Count |
| --- | ---: |
| `<reason>` | `<n>` |

Artifacts:

| Artifact | Path |
| --- | --- |
| predictions | `<path>` |
| raw model results | `<path>` |
| policy eval | `<path>` |
| policy details | `<path>` |

## 12. Baseline Comparison

| Baseline | Metric | Baseline | This run | Delta |
| --- | --- | ---: | ---: | ---: |
| `<baseline>` | exact action accuracy | `<value>` | `<value>` | `<delta>` |

Classification:

```text
improved / regressed / stable_correct / stable_gap
```

## 13. De-Anonymized Eval And MCP Replay

| Check | Value |
| --- | ---: |
| de-anonymized predictions | `<n>` |
| raw policy exact accuracy | `<value>` |
| replay limit smoke | `<n>` |
| replay full rows | `<n>` |
| MCP tool successes | `<n>` |
| MCP failures | `<n>` |
| missing expected refs | `<n>` |
| partial result rows | `<n>` |

Artifacts:

| Artifact | Path |
| --- | --- |
| raw predictions | `<path>` |
| raw policy eval | `<path>` |
| replay summary | `<path>` |
| replay results | `<path>` |

## 14. Final Decision

Final status:

```text
promoted / baseline-only / internal-only / failed / quarantined / aborted
```

Reason:

```text
<One or two paragraphs explaining the decision.>
```

Claims allowed from this run:

- `<claim>`;
- `<claim>`.

Claims not allowed:

- `<claim>`;
- `<claim>`.

## 15. Follow-Up

| Priority | Task | Reason |
| --- | --- | --- |
| P0 | `<task>` | `<reason>` |
