# 2026-05-16 Separated Read/Writer Qwen 0.5B Run

Status: internal-only
Date: 2026-05-16

## 1. Purpose

Run the two Operator profiles separately instead of mixing them:

- `read-api-mcp-v1`: full read-side API/MCP conformance.
- `writer-pre-read-v3`: bounded read decisions before a writer commits memory.

The goal was to measure whether the 0.5B Operator can learn each profile
without mutilating the KMP/MCP surface exposed to that profile.

## 2. Model And Recipe

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

## 3. Reader Result

Dataset:

- export: `/tmp/kernel-operator-conformance-read-api-mcp-v1-20260516`
- SFT: `/tmp/kernel-operator-sft-read-api-mcp-v1-20260516`
- eval rows: 215

Training finished cleanly:

| Metric | Value |
| --- | ---: |
| train runtime | 478.3 s |
| final eval loss | 0.009435 |
| final eval token accuracy | 0.9966 |
| prediction failures | 0 / 215 |

Strict model-facing policy eval:

| Metric | Value |
| --- | ---: |
| exact action | 214 / 215, 99.53% |
| action type | 100% |
| tool | 100% |
| primary refs | 100% |
| scope/about | 100% |
| cursor mode | 100% |
| window | 100% |
| limit | 100% |
| trace page continuation | 100% |
| stop | 100% |
| invalid predictions | 0 |
| unbounded calls | 0 |

Only failure:

```text
read-api-quota-enforcement-conflict-18-stop-sufficient
```

The model selected `stop` with the right refs and policy but omitted one
underscore in the free-form `reason` value. This is a real exact-match failure,
but not an API/MCP coverage failure.

## 4. Writer V3 Diagnostic

The first writer run used `/tmp/kernel-operator-sft-writer-pre-read-v3-20260516`.
It produced valid JSON but scored only 81 / 108 exact actions under the corrected
model-facing evaluator.

The important finding was dataset-side:

```text
all 27 non-exact actions were kernel_near rows
```

The target expected:

```json
{"limit":{"entries":8,"tokens":1200},"window":{"before_entries":4,"after_entries":0}}
```

but the model input did not expose `requested_bounds`. The system prompt showed
only the generic `kernel_near` shape. That made the label depend on an implicit
policy hidden from the model.

This cut is retained as a diagnostic, not as a training claim.

## 5. Writer V3b Correction

Generator change:

```text
writer-pre-read near rows now expose visible_state.requested_bounds
```

This keeps the profile honest: if the target action requires exact bounds, the
model must be able to see the requested bounds or the dataset must explicitly
teach a sizing policy. V3b chooses the former because this is a conformance set.

Dataset:

- export: `/tmp/kernel-operator-conformance-writer-pre-read-v3b-20260516`
- SFT: `/tmp/kernel-operator-sft-writer-pre-read-v3b-20260516`
- eval rows: 108

Training finished cleanly:

| Metric | Value |
| --- | ---: |
| train runtime | 162.3 s |
| final eval loss | 0.02406 |
| final eval token accuracy | 0.9935 |
| prediction failures | 0 / 108 |

Strict model-facing policy eval:

| Metric | Value |
| --- | ---: |
| exact action | 107 / 108, 99.07% |
| action type | 99.07% |
| tool | 100% |
| primary refs | 100% |
| scope/about | 100% |
| cursor mode | 100% |
| window | 100% |
| limit | 100% |
| trace page continuation | 100% |
| stop | 20 / 21, 95.24% |
| invalid predictions | 0 |
| unbounded calls | 0 |

Only failure:

```text
writer-v3-support-sla-repair-30-question-stop-sufficient
```

Expected `stop`; predicted `kernel_trace`. This is a real policy failure:
`remaining_budget.tool_calls` was already `0` and visible evidence was
sufficient. The next dataset slice should add more contrastive stop-vs-trace
rows where:

- last tool is `kernel_trace`;
- page is complete;
- evidence is sufficient;
- remaining tool calls are `0`;
- trace temptation is high because endpoints are visible.

## 6. Evaluator Correction

Initial policy eval compared predictions generated from anonymized prompts
against raw trajectories. That made correct model-facing predictions such as:

```text
about_0001, ref_0001
```

look wrong against raw ids such as:

```text
incident:writer-pre-read-v3:...
```

The evaluator now has an explicit target source:

```bash
underpass_operator_policy_eval --model-facing-eval <eval.jsonl>
```

This fails fast if the SFT row does not contain parseable user and assistant
messages. Raw trajectory evaluation remains available through:

```bash
underpass_operator_policy_eval --trajectories <trajectories.jsonl>
```

The two modes must not be mixed.

## 7. Artifact Hashes

| Artifact | SHA-256 |
| --- | --- |
| reader model-facing policy eval | `a6daf4bbc986d64f67cbf035b04f74a19ac6b31fd6cd9f9454d1585eb3748cd3` |
| reader predictions | `900793b4091aba19796c63cd6816a6d601c545dc295e319cecc804cd1d7f2bf4` |
| writer v3b conformance summary | `39a68f16b2b9d6d1d2823671e760695d071e7b841fa0bec1886d3f6854450340` |
| writer v3b SFT summary | `0fafc15058175c8018ae7bb5aa36949e91dc1ae667a4975bc1e0996702db2498` |
| writer v3b model-facing policy eval | `fe79bd0e1b8429d3a657e8d76808cd6a2429792c8bfab015d2d86d716951bebc` |
| writer v3b predictions | `19d92768d4d0dbf43735010c511341a9a174632d765bb65c34bdbadf93048fd3` |

## 8. Decision

The separated profile approach is validated.

The 0.5B Operator can learn the read and writer-pre-read KMP/MCP action
contracts when:

- each profile has its own prompt and tool surface;
- train/eval model-facing overlap is zero;
- refs are anonymized consistently;
- policy eval uses the same target namespace as prediction;
- explicit copy decisions are visible in the row.

Next step:

```text
Add a focused stop-vs-trace hard-negative slice for writer-pre-read.
```
