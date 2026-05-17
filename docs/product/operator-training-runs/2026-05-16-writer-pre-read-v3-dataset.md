# Operator Training Run: `writer-pre-read-v3-dataset`

Status: `quarantined`

Date opened: 2026-05-16
Owner: Tirso / Codex

## 1. Scope

| Field | Value |
| --- | --- |
| Attempt id | `writer-pre-read-v3-dataset` |
| Profile | `writer-pre-read` |
| Model | not trained in this cut |
| Dataset root | `/tmp/kernel-operator-sft-writer-pre-read-v3-20260516` |
| Artifact root | `../rehydration-kernel-artifacts/operator/2026-05-16-writer-pre-read-v3-dataset/` |
| Branch | `codex/operator-writer-pre-read-v2-train` |

This cut creates the synthetic conformance data for the writer pre-read
operator profile.

Quarantine note, 2026-05-16: training showed that `kernel_near` rows expected
specific bounds without exposing `visible_state.requested_bounds`. The corrected
cut is recorded as v3b in
[`2026-05-16-separated-read-writer-qwen05.md`](2026-05-16-separated-read-writer-qwen05.md).
Keep this document for traceability, but do not use the v3 dataset for training
claims.

It does not train semantic writing. It trains the small Operator to decide how
to read KMP before a writer commits memory.

## 2. Boundary

The writer flow has two different responsibilities:

```text
writer / strong model:
  decides the new memory, relation meaning, why, evidence, and whether rich
  relation proof exists.

Operator:
  chooses the next bounded KMP/MCP action needed to gather enough context.
```

This dataset is only for the Operator side.

Allowed actions in this profile:

- `kernel_near`;
- `kernel_inspect`;
- `kernel_trace`;
- `stop`.

Intentionally absent from the prompt:

- `kernel_wake`;
- `kernel_ask`;
- `kernel_goto`;
- `kernel_rewind`;
- `kernel_forward`;
- `kernel_write_memory`;
- `kernel_ingest`.

The absence of `kernel_write_memory` is deliberate. This profile stops when the
visible context is enough for a writer to decide the next memory relation. It
does not ask the 0.5B model to author that relation.

## 3. Dataset Generation

Conformance export:

```bash
cargo run -p underpass-operator-synthetic-cli --bin underpass_operator_conformance_trajectory_build -- \
  --suite writer-pre-read-v3 \
  --run-id kmp-operator-writer-pre-read-v3-20260516 \
  --output /tmp/kernel-operator-conformance-writer-pre-read-v3-20260516 \
  --force
```

Contract coverage:

```bash
cargo run -p underpass-operator-evaluation-cli --bin underpass_operator_contract_coverage -- \
  --profile writer-pre-read \
  --trajectories /tmp/kernel-operator-conformance-writer-pre-read-v3-20260516/trajectories.jsonl \
  --fail-under 100 \
  --output /tmp/kernel-operator-conformance-writer-pre-read-v3-20260516/contract_coverage_writer_pre_read.json
```

SFT preparation:

```bash
python scripts/operator/prepare_operator_sft_dataset.py \
  --trajectories /tmp/kernel-operator-conformance-writer-pre-read-v3-20260516/trajectories.jsonl \
  --output /tmp/kernel-operator-sft-writer-pre-read-v3-20260516 \
  --include-mode write_context_read \
  --eval-ratio 0.3 \
  --split-mode group \
  --group-key task_or_step \
  --seed 42 \
  --anonymize-refs \
  --require-visible-target-refs \
  --require-visible-target-cursors \
  --capability-split-profile writer-pre-read \
  --require-eval-capability-coverage \
  --require-train-capability-coverage \
  --min-train-capability-count 20 \
  --min-eval-capability-count 10 \
  --max-duplicate-model-row-count 2 \
  --drop-eval-model-row-overlap \
  --force
```

Leak audit:

```bash
python scripts/operator/audit_operator_sft_no_gold.py \
  /tmp/kernel-operator-sft-writer-pre-read-v3-20260516/openai_train.jsonl \
  /tmp/kernel-operator-sft-writer-pre-read-v3-20260516/openai_eval.jsonl \
  --output /tmp/kernel-operator-sft-writer-pre-read-v3-20260516/no_gold_audit.json
```

## 4. Coverage

| Metric | Value |
| --- | ---: |
| trajectories | 360 |
| profile contract coverage | 21 / 21, 100% |
| target capability coverage | 21 / 21, 100% |
| contract validation failures | 0 |
| unsupported tools for this profile | `kernel_ingest`, `kernel_write_memory` |

Tool distribution:

| Target action | Rows |
| --- | ---: |
| `kernel_trace` | 144 |
| `kernel_near` | 72 |
| `kernel_inspect` | 72 |
| `stop` | 72 |

Capability details:

| Capability family | Coverage |
| --- | --- |
| mode | `write_context_read` |
| cursor modes | `ref` |
| dimension modes | `all` |
| dimension scopes | `current_about` |
| trace pagination | first page and continuation |
| window policy | expand, shrink, stop-sufficient |
| writer state | last tool none, near, inspect, trace |
| candidate role | previous subtask answer, same subtask question |
| candidate pool | ambiguous |
| inspect security | `include.raw=false` |

## 5. SFT Quality Gates

| Gate | Observed | Pass |
| --- | ---: | --- |
| selected rows | 360 | yes |
| train rows | 252 | yes |
| eval rows | 108 | yes |
| unique model-facing rows | 360 | yes |
| duplicate model-row hashes | 0 | yes |
| max duplicate model-row count | 1 | yes |
| train/eval model-row overlap | 0 | yes |
| dropped eval overlap rows | 0 | yes |
| dropped non-visible target refs | 0 | yes |
| dropped non-visible target cursors | 0 | yes |
| train capability coverage | 21 / 21, 100% | yes |
| eval capability coverage | 21 / 21, 100% | yes |
| no-gold findings | 0 | yes |

Prompt/tool parity:

| Field | Value |
| --- | --- |
| operator prompt profile | `writer-pre-read` |
| forbidden visible tools | none |
| visible KMP tools | `kernel_near`, `kernel_trace`, `kernel_inspect` |
| non-tool action | `stop` |

This closes the previous collapse risk from the failed diagnostic cut: the model
now sees many distinct writer pre-read states instead of repeated
model-facing rows dominated by `kernel_inspect`.

## 6. Reader vs Writer Comparison

The paired reader dataset is
[`2026-05-16-read-api-mcp-v1-dataset.md`](2026-05-16-read-api-mcp-v1-dataset.md).

| Dimension | Reader `read-api-mcp-v1` | Writer `writer-pre-read-v3` |
| --- | --- | --- |
| goal | operate the full read API/MCP surface | gather context before a smart write |
| mode | `read` | `write_context_read` |
| rows | 716 | 360 |
| train / eval | 501 / 215 | 252 / 108 |
| unique model-facing rows | 716 / 716 | 360 / 360 |
| duplicate model rows | 0 | 0 |
| train/eval overlap | 0 | 0 |
| no-gold findings | 0 | 0 |
| prompt profile | `read` | `writer-pre-read` |
| profile coverage | 24 / 24 | 21 / 21 |
| write tools visible | no | no |

Shared qualities:

- same SFT row contract;
- same anonymized refs;
- same visible-ref and visible-cursor gates;
- same no-gold auditor;
- same duplicate and train/eval overlap gates;
- same profile-specific prompt/tool parity;
- same strict KMP/MCP action validator.

Main differences:

- Reader covers the broad API: wake, ask, temporal movement, trace, inspect,
  dimension scopes, page continuation, and stop.
- Writer pre-read covers the narrower loop needed before writing: near,
  inspect, trace, trace continuation, stop, ambiguous candidates, and
  sufficient-context detection.
- Reader includes multi-about scope and `ALL_ABOUTS`; writer pre-read remains
  current-about because relation proof should be gathered around the memory
  being written unless a later writer policy explicitly asks for cross-about
  proof.
- Reader teaches how to retrieve context for a question. Writer pre-read
  teaches when enough context exists for another component to decide the memory
  relation.

Training implication:

```text
Do not treat writer-pre-read as a replacement for reader.
Train/evaluate reader first, then add writer-pre-read as a second profile or a
profile-conditioned extension.
```

The two sets are cohesive because they share the same contract and quality
gates, but they should remain distinguishable through `mode`,
`operator_prompt_profile`, and `allowed_tools`.

## 7. Artifact Hashes

| Artifact | SHA-256 |
| --- | --- |
| conformance `summary.json` | `408dd4595f05053577e20821e5a1f8c304f1e6182c791a69876232828f9d6845` |
| conformance `contract_coverage_writer_pre_read.json` | `b3f7cdb9069f47a7c34acff2810ef54bf79068f93a028646010ff83d074ab375` |
| conformance `trajectories.jsonl` | `268131ba4d967ee5a644a14c20f64a138def73cb29b2c3f3ef330a5a971220c9` |
| SFT `summary.json` | `ebcdc3e5291acb7932f48be3be4ecae29b843bfe6ebfa48cc1f267b76a1d6b81` |
| SFT `no_gold_audit.json` | `dbb566bbf862f0f3c9bc94eee5f1d5906ad79e00d77e0a1c09c5877d28ee8a02` |
| SFT `openai_train.jsonl` | `9a22a511a44fe2201f71757757b09731b96878b1134d6215da7e36b88f714ed8` |
| SFT `openai_eval.jsonl` | `c421172560d55eb6029855c7242d784ec9e140450e39678362560796fa249025` |

## 8. Decision

This dataset is not ready for controlled training anymore. It is quarantined
because it hid required bounds from the model-facing prompt. Use a corrected
v3b/v4 or later writer pre-read cut instead.

It should not be used to claim full writer capability. Full writer capability
requires a separate dataset where a strong teacher or human writer produces the
semantic relation, `why`, evidence, fallback/escalation decision, and prepared
write payload.
