# 2026-05-16 Writer Exec V1 Dataset

Status: baseline-only
Date: 2026-05-16

## 1. Purpose

Create the first separate dataset for prepared write execution.

This is not `writer-pre-read`. The pre-read profile decides how to inspect KMP
before a writer commits memory. This profile starts after that point:

```text
the writer already prepared a payload -> Operator decides execute or stop
```

The 0.5B Operator is not trained here to invent text, relations, `why`, or
evidence. It learns whether a visible prepared payload can be safely sent to
KMP/MCP, and which write tool to use.

## 2. Profile Boundary

Prompt profile:

```text
writer-exec
```

Visible tools:

```text
kernel_write_memory
kernel_ingest
```

No read tools are exposed. The model may also return `stop`.

The intended policy is:

| State | Correct action |
| --- | --- |
| complete `draft_write.prepared_arguments` | emit `prepared_tool_call` for `kernel_write_memory` |
| complete `canonical_payload` | emit `prepared_tool_call` for `kernel_ingest` |
| no prepared payload | `stop` |
| rich relation without read-context proof | `stop` |
| unsupported/vague relation | `stop` |
| prepared payload `about` mismatch | `stop` |
| incomplete canonical payload | `stop` |
| duplicate idempotency key | `stop` |
| non-strict write options | `stop` |

## 3. Synthetic Shape

Suite:

```text
writer-exec-v1
```

The dataset uses 18 cohesive incident topics. Each topic has the same 13
decision families, so the model sees repeated structure with one policy
boundary changed at a time.

| Family | Rows | Target |
| --- | ---: | --- |
| `write_rich_chosen_because` | 18 | `kernel_write_memory` |
| `write_rich_contradicts` | 18 | `kernel_write_memory` |
| `write_anemic_follows` | 18 | `kernel_write_memory` |
| `write_updates_state` | 18 | `kernel_write_memory` |
| `ingest_single` | 18 | `kernel_ingest` |
| `ingest_multidimensional` | 18 | `kernel_ingest` |
| `missing_prepared_payload` | 18 | `stop` |
| `missing_read_context_proof` | 18 | `stop` |
| `invalid_relation` | 18 | `stop` |
| `about_scope_mismatch` | 18 | `stop` |
| `incomplete_canonical_payload` | 18 | `stop` |
| `duplicate_idempotency` | 18 | `stop` |
| `strict_required` | 18 | `stop` |

Export summary:

| Metric | Value |
| --- | ---: |
| trajectories | 234 |
| mode `write` | 234 |
| target `kernel_write_memory` | 72 |
| target `kernel_ingest` | 36 |
| target `stop` | 126 |
| contract validation failures | 0 |

## 4. Prepared SFT Cut

Command shape:

```text
prepare_operator_sft_dataset.py
  --include-mode write
  --capability-split-profile writer-exec
  --require-visible-target-refs
  --require-visible-target-cursors
  --require-train-capability-coverage
  --require-eval-capability-coverage
  --min-train-capability-count 10
  --min-eval-capability-count 5
  --max-duplicate-model-row-count 2
  --drop-eval-model-row-overlap
  --anonymize-refs
```

Prepared summary:

| Metric | Value |
| --- | ---: |
| selected rows | 234 |
| train rows | 164 |
| eval rows | 70 |
| unique model-facing rows | 234 |
| duplicate model-row hashes | 0 |
| max duplicate model-row count | 1 |
| train/eval model-row overlap | 0 |
| dropped non-visible target refs | 0 |
| dropped non-visible target cursors | 0 |
| writer-exec capability coverage | 20 / 20 |
| no-gold findings | 0 |

Capability coverage:

| Capability | All | Train | Eval |
| --- | ---: | ---: | ---: |
| `tool:kernel_write_memory` | 72 | 49 | 23 |
| `tool:kernel_ingest` | 36 | 29 | 7 |
| `tool:stop` | 126 | 86 | 40 |
| `write:prepared_arguments_visible` | 162 | 111 | 51 |
| `write:canonical_payload_visible` | 54 | 40 | 14 |
| `write:read_context_proof` | 72 | 49 | 23 |
| `write:relation_quality:rich` | 54 | 39 | 15 |
| `write:relation_quality:anemic` | 18 | 10 | 8 |
| `write:canonical_ingest` | 36 | 29 | 7 |
| `write:failfast` | 126 | 86 | 40 |

Each fail-fast reason appears in both train and eval with the configured minimum
coverage.

## 5. Verification

The model-facing oracle policy eval passes exactly:

| Metric | Value |
| --- | ---: |
| eval rows | 70 |
| exact action | 70 / 70 |
| tool | 30 / 30 |
| primary refs | 30 / 30 |
| scope/about | 30 / 30 |
| stop | 40 / 40 |
| invalid predictions | 0 |
| unbounded calls | 0 |

This proves the prepared eval rows are parseable and target actions comply with
the same strict action contract used for predictions.

## 6. Artifact Paths

| Artifact | Path |
| --- | --- |
| conformance export | `/tmp/kernel-operator-conformance-writer-exec-v1-20260516` |
| SFT dataset | `/tmp/kernel-operator-sft-writer-exec-v1-20260516` |
| no-gold audit | `/tmp/kernel-operator-sft-writer-exec-v1-20260516/no_gold_audit.json` |
| oracle policy eval | `/tmp/kernel-operator-sft-writer-exec-v1-20260516/oracle_policy_eval.json` |

## 7. Artifact Hashes

| Artifact | SHA-256 |
| --- | --- |
| conformance summary | `a0ff1bae370421751ee81060d35b5c03e50c4a70c5c52ff43a4b4de4e4ea0b8f` |
| SFT summary | `3cf26e03aab9a2c51da040955069b28438dc177142a47d2df12a0af1cf575db3` |
| no-gold audit | `de9cc4147ad7e426dace39ede642035a323f758c2f73a0f83579f881aaf0b643` |
| oracle policy eval | `d0eaca6af4fd00ba1bf0ffd10a42a372640996ef6efb4767d35c9502d967f18e` |

## 8. Decision

`writer-exec-v1` is retained as the historical full-payload synthetic cut, not
as the current training target.

Update 2026-05-17: the writer-exec model-facing target is now compact. The
Operator emits `prepared_tool_call` with a source handle, and the deterministic
prepared-payload executor copies the visible payload into the final
`kernel_write_memory` or `kernel_ingest` KMP/MCP call. This keeps byte-exact
payload copying out of the 0.5B model.

Do not train a current Operator candidate on this full-payload target. Use
[`2026-05-17-writer-exec-prepared-payload-executor.md`](2026-05-17-writer-exec-prepared-payload-executor.md)
or the mixed
[`2026-05-17-writer-orchestration-v2-kmp-cursor.md`](2026-05-17-writer-orchestration-v2-kmp-cursor.md)
cut instead.

It is deliberately narrow and product-aligned:

- no read tools;
- no semantic inference;
- no reconstruction of memory payloads;
- explicit fail-fast cases;
- prepared `kernel_write_memory` and canonical `kernel_ingest` kept separate;
- successful writes use `prepared_tool_call`, not full model-generated
  `arguments`;
- strict model-facing prompt parity.

The next step recorded by this historical document was a separate Qwen 0.5B
LoRA run over the profile. That run exposed copy-fidelity as the wrong problem
for a small Operator model, which is why prepared-payload execution superseded
this dataset shape.
