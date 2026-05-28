# Operator Training Run: `read-api-mcp-v1-dataset`

Status: `quarantined`

Date opened: 2026-05-16
Owner: Tirso / Codex

## 1. Scope

| Field | Value |
| --- | --- |
| Attempt id | `read-api-mcp-v1-dataset` |
| Profile | `read` |
| Model | not trained in this cut |
| Dataset root | `/tmp/kernel-operator-sft-read-api-mcp-v1-20260516` |
| Artifact root | `../rehydration-kernel-artifacts/operator/2026-05-16-read-api-mcp-v1-dataset/` |
| Branch | `codex/operator-writer-pre-read-v2-train` |

This cut creates the synthetic conformance data required to train Operator on
the KMP/MCP read surface before returning to writer pre-read.

2026-05-17 follow-up: this original 2026-05-16 SFT cut is quarantined under
the current strict contract because its prepared SFT rows contain symbolic
`kernel_trace.page.cursor` values. The replacement cut is:

```text
../rehydration-kernel-artifacts/operator/2026-05-17-kmp-cursor-contract-refresh/kernel-operator-sft-read-api-mcp-v1-kmp-cursor-20260517
```

The replacement keeps the same 716-row read API/MCP surface, passes trainer and
predictor validate-only preflights, and reports 24/24 read profile target
capability coverage with numeric KMP trace cursors. Use the preserved artifact
root above for current training; `/tmp` was only the regeneration scratch area.

## 2. Boundary

This dataset is deliberately read-only.

Operator is trained to choose one bounded KMP/MCP read action from visible
state:

- `kernel_wake`;
- `kernel_ask`;
- `kernel_near`;
- `kernel_goto`;
- `kernel_rewind`;
- `kernel_forward`;
- `kernel_trace`;
- `kernel_inspect`;
- `stop`.

It is not trained here to write memory, infer relation semantics, author `why`
fields, or build typed ingest payloads.

## 3. Why This Cut Exists

The previous writer-pre-read diagnostic showed that coverage alone is not
enough. A dataset can pass leakage checks and still collapse if most
model-facing rows look the same or if rare actions have too little support.

This cut fixes the read-side foundation first:

- every KMP/MCP read capability is covered;
- every target tool appears in train and eval;
- every model-facing row is unique;
- train and eval have zero exact model-row overlap;
- the system prompt exposes only read tools for the read profile.

Writer pre-read remains important, but it should build on a clean read operator
instead of carrying read-surface gaps forward.

## 4. Dataset Generation

Conformance export:

```bash
cargo run -p underpass-operator-synthetic-cli --bin underpass_operator_conformance_trajectory_build -- \
  --suite read-api-mcp-v1 \
  --run-id kmp-operator-read-api-mcp-v1-20260516 \
  --output /tmp/kernel-operator-conformance-read-api-mcp-v1-20260516 \
  --force
```

Contract coverage:

```bash
cargo run -p underpass-operator-evaluation-cli --bin underpass_operator_contract_coverage -- \
  --profile read \
  --trajectories /tmp/kernel-operator-conformance-read-api-mcp-v1-20260516/trajectories.jsonl \
  --fail-under 100 \
  --output /tmp/kernel-operator-conformance-read-api-mcp-v1-20260516/contract_coverage_read.json
```

SFT preparation:

```bash
python scripts/operator/prepare_operator_sft_dataset.py \
  --trajectories /tmp/kernel-operator-conformance-read-api-mcp-v1-20260516/trajectories.jsonl \
  --output /tmp/kernel-operator-sft-read-api-mcp-v1-20260516 \
  --include-mode read \
  --eval-ratio 0.3 \
  --split-mode group \
  --group-key task_or_step \
  --seed 42 \
  --anonymize-refs \
  --require-visible-target-refs \
  --require-visible-target-cursors \
  --capability-split-profile read \
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
  /tmp/kernel-operator-sft-read-api-mcp-v1-20260516/openai_train.jsonl \
  /tmp/kernel-operator-sft-read-api-mcp-v1-20260516/openai_eval.jsonl \
  --output /tmp/kernel-operator-sft-read-api-mcp-v1-20260516/no_gold_audit.json
```

## 5. Coverage

| Metric | Value |
| --- | ---: |
| trajectories | 716 |
| profile contract coverage | 24 / 24, 100% |
| target capability coverage | 24 / 24, 100% |
| contract validation failures | 0 |
| unsupported tools for read profile | `kernel_ingest`, `kernel_write_memory` |

Tool distribution:

| Target action | Rows |
| --- | ---: |
| `kernel_ask` | 176 |
| `kernel_near` | 132 |
| `kernel_trace` | 88 |
| `kernel_inspect` | 72 |
| `stop` | 72 |
| `kernel_wake` | 44 |
| `kernel_goto` | 44 |
| `kernel_rewind` | 44 |
| `kernel_forward` | 44 |

Capability details:

| Capability family | Coverage |
| --- | --- |
| cursor modes | `ref`, `time`, `sequence` |
| dimension modes | `all`, `only`, `except` |
| dimension scopes | `current_about`, `abouts`, `all_abouts` |
| trace pagination | first page and continuation |
| window policy | expand, shrink, stop-sufficient |
| inspect security | `include.raw=false` |

## 6. SFT Quality Gates

| Gate | Observed | Pass |
| --- | ---: | --- |
| selected rows | 716 | yes |
| train rows | 501 | yes |
| eval rows | 215 | yes |
| unique model-facing rows | 716 | yes |
| duplicate model-row hashes | 0 | yes |
| max duplicate model-row count | 1 | yes |
| train/eval model-row overlap | 0 | yes |
| dropped eval overlap rows | 0 | yes |
| dropped non-visible target refs | 0 | yes |
| dropped non-visible target cursors | 0 | yes |
| train capability coverage | 24 / 24, 100% | yes |
| eval capability coverage | 24 / 24, 100% | yes |
| no-gold findings | 0 | yes |

Prompt/tool parity:

| Field | Value |
| --- | --- |
| operator prompt profile | `read` |
| forbidden visible tools | none |
| visible tools | `kernel_wake`, `kernel_ask`, `kernel_near`, `kernel_goto`, `kernel_rewind`, `kernel_forward`, `kernel_trace`, `kernel_inspect` |

This closes the gap where read-only training previously saw write tool shapes in
the generic system prompt.

## 7. Artifact Hashes

| Artifact | SHA-256 |
| --- | --- |
| conformance `summary.json` | `46f1c8e84df9e69760b2db0c4d9d1691f4807d9f6cd1ba059791dd7911ea510b` |
| conformance `contract_coverage_read.json` | `de79c411b1bb48880e53c341d27de0d63d7b6f1fddd09fe94615df3907d0eaaa` |
| conformance `trajectories.jsonl` | `eb1f9b0ac74da986804c27e94289e7b6887d7b9d65eda5b352c16a4202f20f84` |
| SFT `summary.json` | `068e7be8792fb2a9db9806400aaeb8f6faba2a9b5c0a4c54ac57efed3c0ca3fd` |
| SFT `no_gold_audit.json` | `c36923d0b76dd35c1dc980826600cecfbd51bf2130eb3a13d320c44a48791762` |
| SFT `openai_train.jsonl` | `aee826d29a834d31e2ee12a7105a907fe7ee4a7a1088cdb03bacd27c674299a0` |
| SFT `openai_eval.jsonl` | `dffc8bbc41ce248bce362acb7f20d295addf2d2a4a41b3b3864dd0e47238f6e8` |

## 8. Decision

This dataset is ready for a controlled read-profile training run.

It should not be used to claim writer capability. The next training attempt
should train/evaluate read first, then add writer-pre-read as a separate
profile once the read operator is stable.

## 9. Verification

```bash
cargo test -p underpass-operator-synthetic-cli --bin underpass_operator_conformance_trajectory_build
```

Result:

```text
9 passed; 0 failed
```
