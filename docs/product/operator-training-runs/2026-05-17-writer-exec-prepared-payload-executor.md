# 2026-05-17 Writer Exec Prepared-Payload Executor

Status: dataset-ready
Date: 2026-05-17

## 1. Purpose

Move byte-exact payload copying out of the 0.5B Operator model.

The previous `writer-exec` run showed that the model can choose the correct
write action, scope, refs, and stop policy, but it rewrote one word inside a
prepared payload. That is not acceptable for a prepared-write execution profile.

The new boundary is:

```text
Operator -> chooses prepared payload source
executor -> copies prepared payload exactly into final KMP/MCP call
KMP      -> validates and executes kernel_write_memory/kernel_ingest
```

## 2. Model-Facing Action

Successful writer execution now uses compact Operator-side actions:

```json
{"action":{"type":"prepared_tool_call","tool":"kernel_write_memory","source":"draft_write.prepared_arguments"}}
```

or:

```json
{"action":{"type":"prepared_tool_call","tool":"kernel_ingest","source":"canonical_payload"}}
```

`prepared_tool_call` is not a public KMP/MCP tool. It is resolved before the
real API call.

## 3. Deterministic Resolution

The resolver accepts the compact action plus the model-facing `visible_state`.

Resolution rules:

| Compact action | Source | Final action |
| --- | --- | --- |
| `kernel_write_memory` | `visible_state.draft_write.prepared_arguments` | `tool_call:kernel_write_memory` |
| `kernel_ingest` | `visible_state.canonical_payload` | `tool_call:kernel_ingest` |

The resolver fails fast if:

- the source is missing;
- the source is not an object;
- the payload `about` differs from the top-level `about`;
- the resolved action violates the KMP action contract.

This keeps semantic writing and payload construction outside the small model,
while preserving deterministic, auditable execution.

## 4. Implementation

Changed surfaces:

| Surface | Change |
| --- | --- |
| SFT dataset preparer | `writer-exec` assistant targets are projected to `prepared_tool_call` |
| writer-exec prompt | model is instructed not to emit full `arguments` |
| predictor | `--resolve-prepared-payloads` expands compact actions into final KMP actions |
| policy evaluator | `--resolve-prepared-payloads` resolves targets and predictions before scoring |
| tests | policy eval verifies compact target/prediction resolves to exact final KMP action |

## 5. Regenerated Dataset

Output:

```text
/tmp/kernel-operator-sft-writer-exec-v1-prepared-exec-20260517
```

Summary:

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

Resolved eval still sees the final KMP action distribution:

| Final action | Count |
| --- | ---: |
| `stop` | 40 |
| `kernel_write_memory` | 23 |
| `kernel_ingest` | 7 |

## 6. Decision

This replaces the copy-fidelity training objective for `writer-exec`.

The model should not learn to reproduce long payload JSON. It should learn:

- execute prepared write;
- execute prepared ingest;
- stop when execution is unsafe.

Exact copying belongs to the deterministic executor.

The next training run should use this regenerated dataset and run prediction
with `--resolve-prepared-payloads`.

## 7. Artifact Hashes

| Artifact | SHA-256 |
| --- | --- |
| SFT summary | `09bf5a772be93bb4c339b5b5a67e272089c58de5a04f4520c98ccdaf03cbf404` |
| no-gold audit | `8508c8335467ac8c656c7c7ad980d1057ab2df11bd3937d23315d3673bd10655` |
| resolved oracle policy eval | `a169772e8b2b2cc5e68dfa40f030fe248a1fd1ddf71b37e27f8cfe3042272233` |
