# Kernel Operator Training

This folder contains the external training path for a small KMP tool-operator
model. The model is not part of kernel core. It learns to emit one bounded
KMP/MCP action from a visible memory state.

Current training-data status, 2026-05-17:

- keep previous LoRA runs as baselines;
- use the MemoryArena P1.11 corpus as the clean current training base;
- use `read-api-mcp-v1-kmp-cursor-20260517` as the synthetic read API/MCP
  conformance base before placing Operator in front of the full read surface;
- use the KMP-cursor writer orchestration cut as the current writer base:
  `writer-pre-read-v4-kmp-cursor-v2` plus
  `writer-exec-v1-kmp-cursor-v2`;
- keep the older standalone `read-api-mcp-v1` and `writer-pre-read-v4`
  manifests quarantined because their SFT rows used symbolic trace cursors;
- keep `writer-orchestration-v1` quarantined for claims because it used
  symbolic trace cursors and weaker stop/prediction validation;
- treat mixed MemoryArena + LongMemEval data as internal comparison only;
- do not train publication candidates from the LongMemEval-S cleaned 500
  full-history artifacts until repeated `session_id` semantics are explicitly
  modeled;
- fail fast on unsupported benchmark shapes instead of generating fallback ids.
- require prompt/tool parity: a read-profile SFT dataset must expose only read
  tools in the system prompt, while write tools remain outside that profile.
- require trace pagination parity: `kernel_trace.page.cursor` must be the
  numeric string returned by KMP `Trace.next_cursor`, not a synthetic cursor
  label such as `page:next` or `trace:page:2`.
- require exact public KMP/MCP enum and relation wire values; Operator datasets
  must not rely on trimming, lowercasing, relation aliases, or silent
  canonicalization.
- require write-shape visibility in reports: dry-run versus commit, strict
  options, prepared payload defaults, `kernel_ingest` dimensions/relations/
  evidence/provenance presence, and raw access exclusions must be explicit.
- treat the FunctionGemma-native scripts as legacy/read-only experiments. They
  now fail fast on `write`, `write_context_read`, `prepared_tool_call`, writer
  datasets, and target tools outside their small native function schema.

See
[`docs/product/operator-training-data-audit-2026-05-13.md`](../../docs/product/operator-training-data-audit-2026-05-13.md)
for the current classification.

Current action-contract status, 2026-05-14:

- previous Operator policy metrics are `pre-strict` unless revalidated with the
  shared action validator;
- a prediction is valid only if it matches the exact KMP/MCP action schema;
- fields that belong to `stop`, such as `final_refs`, are invalid inside
  `tool_call.arguments`;
- the predictor now rejects out-of-contract actions before writing
  `predictions.jsonl`;
- publication candidates must have zero invalid predictions and zero unbounded
  calls under the strict validator.

See
[`docs/product/operator-action-contract-audit-2026-05-14.md`](../../docs/product/operator-action-contract-audit-2026-05-14.md)
for impact, corrected v10 metrics, and the required revalidation plan.

Current revalidation result:

- MemoryArena V6 holdout20 remains clean under
  `kernel-operator-action-contract-v1`: 1,124/1,124 exact, zero missing, zero
  invalid, zero unbounded, both anonymized and de-anonymized.
- The same strict raw predictions replayed through the public TLS MCP/gRPC
  endpoint: 976/976 tool calls succeeded, 148 stop actions, zero missing
  expected refs, and 424 explicit partial results from `kernel_near`.
- LongMemEval v8 clean is internal only under the strict contract: 4 missing
  predictions, 2 invalid predictions, 0 unbounded, 0.7500 exact.

Current contract-coverage status:

- `operator-read` contract coverage is 100% after adding `kernel_wake`,
  time/sequence temporal cursors, full dimension mode/scope validation, trace
  pagination, and window-policy capability checks.
- `read-api-mcp-v1-kmp-cursor-20260517` generates 716 strict read
  trajectories with 100% read profile coverage, 716 unique model-facing SFT
  rows, zero duplicate row hashes, zero train/eval model-row overlap, and
  numeric KMP trace cursors.
- `writer-pre-read-v3` is historical only. It exposed useful coverage but was
  superseded by v3b/v4 and then by the KMP-cursor v2 cut.
- The current mixed writer-orchestration dataset has 810 rows, 40/40
  train/eval capability coverage, 0 duplicate model rows, 0 train/eval
  overlap, 0 dropped hidden refs/cursors, numeric KMP trace cursors, strict
  stop validation, and 243/243 resolved oracle exact eval.
- `operator-full` contract coverage is 100%; the contract now includes
  `kernel_ingest`, `kernel_write_memory`, relation-quality validation, and
  read-context proof.
- MemoryArena V6 target capability coverage is only 41.67% for the read
  profile. The model has not yet seen all API/MCP use cases.
- MemoryArena V6 target capability coverage is 35.71% for the full profile
  because it has not yet seen write actions.
- The synthetic KMP conformance exporter now produces 58 strict trajectories
  that cover 100% of the `operator-full` target capabilities, including
  `kernel_write_memory`, `kernel_ingest`, relation quality, read-context proof,
  trace pagination, temporal cursor modes, dimension modes/scopes, dynamic
  window cases, stop decisions, and write/read fail-fast behavior.
- The conformance SFT prompt now includes the top-level `goal`. Earlier
  conformance predictions without `goal` are diagnostic only: they exposed a
  dataset-preparation gap, not a stable model-quality result.
- The v4 conformance SFT path exposed a second dataset problem: some write
  samples required the model to invent `kernel_ingest`/`kernel_write_memory`
  payloads that were not visible in the prompt.
- The v5 conformance SFT path fixes that corpus honesty issue by keeping
  `about_*` separate from node `ref_*`, by exposing `canonical_payload` for
  canonical ingest tests, and by exposing `draft_write.prepared_arguments` for
  prepared write tests.
- Do not train a public `operator-full` writer yet. The next public training
  target is still `operator-read`; write samples are contract/anti-invention
  tests until the smart-writer flow is designed separately.
- Treat the 0.5B model as a strict Kernel Operator, not as a semantic relation
  author. It should only learn how to use KMP: bounded tool calls,
  read-before-write policy, prepared-write execution, escalation decisions, and
  strict JSON emission. Rich writer relation labels should come from an offline
  strong teacher dataset, with GPT-5.5 as the preferred teacher for this line.

Coverage command:

```bash
cargo run -p underpass-operator-evaluation-cli --bin underpass_operator_contract_coverage -- \
  --profile read \
  --trajectories /tmp/kernel-operator-sft-100-with-writer-by-task-anon-visible-candidate-details-holdout20/eval_trajectories.jsonl
```

Conformance trajectory command:

```bash
cargo run -p underpass-operator-synthetic-cli --bin underpass_operator_conformance_trajectory_build -- \
  --suite read-api-mcp-v1 \
  --run-id kmp-operator-read-api-mcp-v1-kmp-cursor-20260517 \
  --output /tmp/kernel-operator-conformance-read-api-mcp-v1-kmp-cursor-20260517 \
  --force

cargo run -p underpass-operator-evaluation-cli --bin underpass_operator_contract_coverage -- \
  --profile read \
  --trajectories /tmp/kernel-operator-conformance-read-api-mcp-v1-kmp-cursor-20260517/trajectories.jsonl \
  --fail-under 100

cargo run -p underpass-operator-synthetic-cli --bin underpass_operator_conformance_trajectory_build -- \
  --suite writer-pre-read-v4 \
  --run-id kmp-operator-writer-pre-read-v4-kmp-cursor-v2-20260517 \
  --output /tmp/kernel-operator-conformance-writer-pre-read-v4-kmp-cursor-v2-20260517 \
  --force

cargo run -p underpass-operator-evaluation-cli --bin underpass_operator_contract_coverage -- \
  --profile writer-pre-read \
  --trajectories /tmp/kernel-operator-conformance-writer-pre-read-v4-kmp-cursor-v2-20260517/trajectories.jsonl \
  --fail-under 100

cargo run -p underpass-operator-synthetic-cli --bin underpass_operator_conformance_trajectory_build -- \
  --output /tmp/kernel-operator-conformance-full-v5 \
  --force

cargo run -p underpass-operator-evaluation-cli --bin underpass_operator_contract_coverage -- \
  --profile full \
  --trajectories /tmp/kernel-operator-conformance-full-v5/trajectories.jsonl \
  --fail-under 100
```

Conformance SFT preparation:

```bash
python scripts/operator/prepare_operator_sft_dataset.py \
  --trajectories /tmp/kernel-operator-conformance-full-v5/trajectories.jsonl \
  --output /tmp/kernel-operator-conformance-full-v5-sft \
  --eval-ratio 0.25 \
  --seed 42 \
  --anonymize-refs \
  --require-visible-target-refs \
  --require-visible-target-cursors \
  --force

python scripts/operator/audit_operator_sft_no_gold.py \
  /tmp/kernel-operator-conformance-full-v5-sft/openai_train.jsonl \
  /tmp/kernel-operator-conformance-full-v5-sft/openai_eval.jsonl \
  --output /tmp/kernel-operator-conformance-full-v5-sft/no_gold_audit.json
```

Current conformance SFT output:

| Metric | Value |
| --- | ---: |
| selected rows | 58 |
| train rows | 44 |
| eval rows | 14 |
| read rows | 42 |
| write rows | 16 |
| dropped non-visible target refs | 0 |
| dropped non-visible target cursors | check for new golden cuts |
| debug audit | `<output>/debug_audit.jsonl` |
| no-gold audit findings | 0 |
| full target capability coverage on `all_model_trajectories` | 100% |
| `goal` present in model-facing user prompt | yes |

Current Qwen2.5-0.5B conformance smoke history:

| Metric | Value |
| --- | ---: |
| adapter | `/tmp/kernel-operator-qwen05-lora-conformance-full-v2` |
| prediction output | `/tmp/kernel-operator-qwen05-conformance-full-v2-predictions` |
| policy eval | `/tmp/kernel-operator-qwen05-conformance-full-v2-policy-eval.json` |
| training epochs | 8 |
| final eval loss | 0.06752 |
| final eval mean token accuracy | 0.9894 |
| valid predictions | 25/30 |
| missing predictions | 5/30 |
| exact action accuracy | 6/30 |
| action type accuracy | 24/30 |
| tool accuracy | 17/27 tool calls |
| primary ref accuracy | 18/27 tool calls |
| scope accuracy | 23/27 tool calls |
| cursor mode accuracy | 4/13 cursor actions |
| window shape accuracy | 10/13 window actions |
| limit policy accuracy | 10/13 limit actions |
| continue page accuracy | 2/2 page continuations |

This was a pipeline/conformance smoke, not a release result. The old v6 adapter
failed the full contract almost completely because it was trained before
`operator-full`; v2 proved that the training path can learn valid full-contract
actions, but a 30-row suite was too small to teach stable tool choice, cursor
choice, dynamic window policy, and strict smart-write behavior.

The v3 run expanded the suite to 58 rows but exposed a more important issue:
the SFT user payload did not include the top-level `goal`. The model saw the
state and allowed tools, but not the actual operator intent. Treat v3
predictions as diagnostic only.

The v4 run keeps the 58-row expanded suite and fixes the SFT prompt by adding
`goal`. Its predictions are diagnostic only for write behavior because the
write/ingest prompts still required invented payload content.

The v5 run keeps the same coverage but fixes the write data shape:

- `about` anonymizes to `about_0001`, not `ref_0001`;
- canonical ingest targets expose the exact `visible_state.canonical_payload`;
- prepared write targets expose the exact
  `visible_state.draft_write.prepared_arguments`;
- no write target requires fields that are absent from visible state.

This does not mean writing is ready for public Operator training. Real KMP
writing requires a writer/LLM to read context first, decide the semantic
relation and `why`, then write a rich relation only when justified. If that
relation is not justified, the writer should use the deterministic anemic
fallback such as `follows`. The kernel validates scope, evidence and audit
proof; it does not infer the relation meaning.

Do not expect a 0.5B Operator to author rich relations from scratch.
For writer data, use a strong offline teacher to produce the semantic decision:
relation, `why`, cited evidence, and explicit fallback/escalation when the proof
is insufficient. The preferred teacher for this track is GPT-5.5, pinned in
dataset provenance as the exact model id used for the run. If that teacher is
not available, writer dataset generation should fail fast rather than silently
switching models.

The 0.5B Operator can still learn valuable kernel-operation behavior around
writer workflows:

- when to call `kernel_near`, `kernel_trace`, or `kernel_inspect` before write;
- when visible context is enough to execute a prepared write payload;
- when a relation needs escalation to the teacher/large reasoning model;
- when only an explicit anemic fallback is allowed;
- how to emit one bounded Operator decision without inventing refs or
  arguments.

For the `writer-exec` and `writer-orchestration` profiles, successful writes
are no longer model-generated full KMP calls. The model emits:

```json
{"action":{"type":"prepared_tool_call","tool":"kernel_write_memory","source":"draft_write.prepared_arguments"}}
```

or:

```json
{"action":{"type":"prepared_tool_call","tool":"kernel_ingest","source":"canonical_payload"}}
```

The deterministic prepared-payload executor resolves that compact decision into
the real `kernel_write_memory` or `kernel_ingest` action by copying the visible
payload exactly and then validating the final action against the KMP contract.
Use `predict_operator_sft.py --resolve-prepared-payloads` during prediction and
`underpass_operator_policy_eval --resolve-prepared-payloads` during model-facing
evaluation.

The `writer-orchestration` profile combines writer pre-read and prepared write
execution in one policy surface. It still does not let the 0.5B model author
semantic write payloads. The model-facing `allowed_tools` list is projected per
row so the prompt and visible contract stay aligned:

- `write_context_read` rows expose only `kernel_near`, `kernel_trace`, and
  `kernel_inspect`;
- `write` rows expose only `kernel_write_memory` and `kernel_ingest`;
- `stop` remains available through the action contract.

The raw trajectory must already match that profile-level tool surface. The SFT
preparer fails fast if `allowed_tools` contains a tool outside the active
profile instead of silently dropping it.

Observed failure classes:

- two `kernel_ask` generations used `dimensions.mode=only` without `include`,
  which the strict validator rejects;
- one `kernel_write_memory` generation omitted the top-level action type;
- one `kernel_write_memory` generation added `semantic_delta` but omitted the
  required `semantic_delta.why`;
- one `kernel_write_memory` generation added an unexpected `strategy` object;
- several `kernel_goto`/`kernel_rewind` targets were predicted as
  `kernel_forward`, showing that temporal direction and cursor-mode selection
  still need more data.

Next P0 before scaling benchmarks: grow the `operator-read` conformance corpus
with multiple variants per capability, train from the v5+ prompt shape, then
require zero strict-output failures before any public read-Operator claim or
live MCP replay. Keep write training separate until the smart-writer design is
closed.

The current shell does not have local inference dependencies installed
(`torch`, `transformers`, `peft`, `accelerate`). Run the LoRA/prediction steps
from the GPU training environment or Kubernetes job used for previous Operator
runs.

## 1. Prepare SFT Data

```bash
python scripts/operator/prepare_operator_sft_dataset.py \
  --trajectories /tmp/kernel-operator-trajectories-100/trajectories.jsonl \
  --output /tmp/kernel-operator-sft-100 \
  --eval-ratio 0.1 \
  --seed 42 \
  --force
```

Harder split by task:

```bash
python scripts/operator/prepare_operator_sft_dataset.py \
  --trajectories /tmp/kernel-operator-trajectories-100-with-writer/trajectories.jsonl \
  --output /tmp/kernel-operator-sft-100-with-writer-by-task \
  --split-mode group \
  --group-key task_id \
  --eval-ratio 0.1 \
  --seed 42 \
  --force
```

Use the grouped split for model claims. The row split is useful only for smoke
tests because it can place adjacent steps from the same task in both train and
eval.

Mode filters are available for profile-specific datasets:

```bash
python scripts/operator/prepare_operator_sft_dataset.py \
  --trajectories <trajectories.jsonl> \
  --output <operator-read-sft-dir> \
  --include-mode read \
  --include-mode write_context_read \
  --split-mode group \
  --group-key task_or_step \
  --anonymize-refs \
  --require-visible-target-refs \
  --force
```

Use `--include-mode read` to create a pure read conformance slice without
`kernel_ingest` or `kernel_write_memory`. Use `task_or_step` when mixing real
benchmark tasks with synthetic conformance rows: real benchmark rows remain
grouped by task/question id, while synthetic rows without task ids fall back to
their `step_id`.

Capability-aware split for Operator read claims:

```bash
python scripts/operator/prepare_operator_sft_dataset.py \
  --trajectories <real-benchmark-trajectories.jsonl> \
  --trajectories <conformance-trajectories.jsonl> \
  --output <operator-read-sft-dir> \
  --include-mode read \
  --split-mode group \
  --group-key task_or_step \
  --eval-ratio 0.1 \
  --seed 42 \
  --anonymize-refs \
  --require-visible-target-refs \
  --capability-split-profile read \
  --require-eval-capability-coverage \
  --require-train-capability-coverage \
  --force
```

Use this for serious `operator-read` training. It orders groups with a stable
seeded hash strategy, seeds eval with groups that cover the declared KMP/MCP
capability profile, preserves benchmark task grouping, and fails before writing
a usable training claim if either train or eval lacks a required capability. If
a required capability appears in fewer than two distinct groups, the command
fails because the same example cannot prove both train exposure and eval
coverage without leakage.

Use a separate profile for smart-writer pre-read. This prevents normal
`operator-read` results from hiding writer pre-read failures, and prevents
writer pre-read rows from blocking a read-profile claim:

```bash
cargo run -p underpass-operator-synthetic-cli --bin underpass_operator_conformance_trajectory_build -- \
  --suite writer-pre-read-v2 \
  --run-id kmp-operator-writer-pre-read-v2-YYYYMMDD \
  --output <writer-pre-read-conformance-dir> \
  --force

cargo run -p underpass-operator-evaluation-cli --bin underpass_operator_contract_coverage -- \
  --profile writer-pre-read \
  --trajectories <writer-pre-read-conformance-dir>/trajectories.jsonl \
  --fail-under 100

python scripts/operator/prepare_operator_sft_dataset.py \
  --trajectories <writer-pre-read-conformance-dir>/trajectories.jsonl \
  --output <writer-pre-read-sft-dir> \
  --include-mode write_context_read \
  --split-mode group \
  --group-key task_or_step \
  --eval-ratio 0.5 \
  --seed 42 \
  --anonymize-refs \
  --require-visible-target-refs \
  --require-visible-target-cursors \
  --capability-split-profile writer-pre-read \
  --require-eval-capability-coverage \
  --require-train-capability-coverage \
  --force
```

The `writer-pre-read` profile requires bounded `near`, `inspect`, `trace`, and
`stop`, ref cursors, current-about dimensions, shrink/expand/stop window
policy, `inspect.raw=false`, first and continuation trace pages, writer
`last_tool` states through `kernel_trace`, candidate roles for previous answers
and same-subtask questions, and explicit ambiguous candidate pools.

`writer-pre-read-v1` and `writer-pre-read-v2` are retained as historical
fixtures. Use `writer-pre-read-v4-kmp-cursor-v2` for new writer pre-read or
writer-orchestration claims because it covers sufficient-context stops, trace
pagination, ambiguous writer candidate decisions, and numeric KMP trace
cursors.

Use `writer-orchestration` when training the combined writer policy that must
decide whether to keep reading memory or execute a prepared write. For the
current clean KMP-cursor cut, the preserved artifact root is:

```bash
ARTIFACT_ROOT=../rehydration-kernel-artifacts/operator/2026-05-17-kmp-cursor-contract-refresh
RUN_ROOT=../rehydration-kernel-artifacts/operator/runs
```

The regeneration recipe is:

```bash
python scripts/operator/prepare_operator_sft_dataset.py \
  --trajectories /tmp/kernel-operator-conformance-writer-pre-read-v4-kmp-cursor-v2-20260517/trajectories.jsonl \
  --trajectories /tmp/kernel-operator-conformance-writer-exec-v1-kmp-cursor-v2-20260517/trajectories.jsonl \
  --output /tmp/kernel-operator-sft-writer-orchestration-v2-kmp-cursor-20260517 \
  --include-mode write_context_read \
  --include-mode write \
  --split-mode group \
  --group-key task_or_step \
  --eval-ratio 0.3 \
  --seed 42 \
  --anonymize-refs \
  --require-visible-target-refs \
  --require-visible-target-cursors \
  --capability-split-profile writer-orchestration \
  --min-train-capability-count 5 \
  --min-eval-capability-count 5 \
  --force
```

Do not train current jobs directly from `/tmp`. After regeneration, preserve
the accepted cut under `ARTIFACT_ROOT` and train from that root. The current
artifact gate checks train/eval coverage for read, writer-pre-read,
writer-exec, and mixed writer-orchestration profiles:

```bash
bash scripts/operator/check_current_operator_artifacts.sh
```

Before launching a GPU job, validate the exact train/eval files with the same
contract gate used by the trainer:

```bash
python scripts/operator/train_operator_sft_lora.py \
  --train-jsonl "$ARTIFACT_ROOT/kernel-operator-sft-writer-orchestration-v2-kmp-cursor-20260517/openai_train.jsonl" \
  --eval-jsonl "$ARTIFACT_ROOT/kernel-operator-sft-writer-orchestration-v2-kmp-cursor-20260517/openai_eval.jsonl" \
  --output-dir "$RUN_ROOT/kernel-operator-qwen05-lora-writer-orchestration-v2-kmp-cursor-validate-only" \
  --validate-only
```

This validates strict action shape, row-level `allowed_tools`, prepared-payload
resolution, duplicate model-facing rows, and train/eval overlap without loading
training dependencies.

Also gate the prepared SFT rows with the Rust API/MCP coverage reporter. The
reporter accepts both raw trajectories and final SFT JSONL rows. For the
combined writer-orchestration cut, both constituent profiles must be 100%:

```bash
cargo run -p underpass-operator-evaluation-cli --bin underpass_operator_contract_coverage -- \
  --profile writer-pre-read \
  --trajectories "$ARTIFACT_ROOT/kernel-operator-sft-writer-orchestration-v2-kmp-cursor-20260517/eval.jsonl" \
  --fail-under 100

cargo run -p underpass-operator-evaluation-cli --bin underpass_operator_contract_coverage -- \
  --profile write \
  --trajectories "$ARTIFACT_ROOT/kernel-operator-sft-writer-orchestration-v2-kmp-cursor-20260517/eval.jsonl" \
  --fail-under 100
```

Use the same preflight for prediction:

```bash
python scripts/operator/predict_operator_sft.py \
  --dataset-jsonl "$ARTIFACT_ROOT/kernel-operator-sft-writer-orchestration-v2-kmp-cursor-20260517/eval.jsonl" \
  --model-id Qwen/Qwen2.5-0.5B-Instruct \
  --output "$RUN_ROOT/kernel-operator-qwen05-predictions-writer-orchestration-v2-kmp-cursor-validate-only" \
  --resolve-prepared-payloads \
  --validate-only
```

This profile requires 100% coverage for both constituent contracts:
writer pre-read navigation and writer-exec fail-fast/prepared-payload
execution. Use this profile before claiming that Operator can sit in the
writer path without hiding part of the KMP/MCP API. Prediction for this profile
must use `--resolve-prepared-payloads`; live replay expects final
`kernel_write_memory` or `kernel_ingest` calls, not model-facing
`prepared_tool_call` placeholders.

For large real benchmark exports, also add model-row quality gates before
training:

```bash
python scripts/operator/prepare_operator_sft_dataset.py \
  --trajectories <real-benchmark-trajectories.jsonl> \
  --trajectories <conformance-trajectories.jsonl> \
  --output <operator-read-quality-sft-dir> \
  --include-mode read \
  --include-mode write_context_read \
  --split-mode group \
  --group-key task_or_step \
  --eval-ratio 0.1 \
  --seed 42 \
  --anonymize-refs \
  --require-visible-target-refs \
  --require-visible-target-cursors \
  --capability-split-profile read \
  --require-eval-capability-coverage \
  --require-train-capability-coverage \
  --min-train-capability-count 5 \
  --min-eval-capability-count 3 \
  --max-duplicate-model-row-count 1 \
  --drop-eval-model-row-overlap \
  --force
```

`--max-duplicate-model-row-count` caps exact duplicate model-facing SFT rows
after prompt construction. `--drop-eval-model-row-overlap` removes eval rows
whose exact model-facing row also appears in train, then rechecks capability
coverage on the final split. Use these gates when anonymization collapses many
real trajectories into the same prompt/answer template.

`--min-train-capability-count` and `--min-eval-capability-count` prevent a false
green dataset where a required API/MCP capability appears only once. For current
`operator-read` release-candidate cuts, use at least 5 train and 3 eval examples
per required capability.

Strict ref-safe split for the current real operator run:

```bash
python scripts/operator/prepare_operator_sft_dataset.py \
  --trajectories /tmp/kernel-operator-trajectories-100-with-writer-candidate-details/trajectories.jsonl \
  --output /tmp/kernel-operator-sft-100-with-writer-by-task-anon-visible-candidate-details \
  --split-mode group \
  --group-key task_id \
  --eval-ratio 0.1 \
  --seed 42 \
  --anonymize-refs \
  --require-visible-target-refs \
  --force
```

Use this mode for training claims. It replaces model-facing refs with synthetic
per-step refs such as `ref_0001`, and drops rows whose target action refers to
refs that are not visible in `current_ref`, `trace_target_ref`,
`candidate_refs`, `candidate_ref_details`, `known_refs`, or
`last_observed_refs`. `candidate_refs` is required for writer context-read rows;
without it, valid writer candidates can look invisible after anonymization.

Requested-field projection is available for contract translation and live replay
smokes:

```bash
python scripts/operator/prepare_operator_sft_dataset.py \
  --trajectories <operator-trajectories>/trajectories.jsonl \
  --output <operator-requested-sft-dir> \
  --include-mode read \
  --split-mode group \
  --group-key task_id \
  --anonymize-refs \
  --require-visible-target-refs \
  --inject-target-request-fields \
  --force
```

This projects the audited `target_action` into model-visible requested fields
such as `requested_wake`, `requested_ask`, `requested_move`,
`requested_scope`, `requested_bounds`, `requested_trace`,
`inspection_request`, and `requested_stop`.

Use it only when the claim is:

```text
Operator translates an explicit upstream KMP request into the strict MCP/API
action contract.
```

Do not use it to claim autonomous benchmark planning. A planner dataset must
make the upstream decision visible through real state, not by projecting the
target action into requested fields.

The current preferred dataset also includes `candidate_ref_details` for writer
context-read rows. These details are structural and model-facing after
anonymization: role, turn kind, relative temporal position, priority, and a
relation hint derived from the entry kind. They intentionally do not expose the
writer's final `connect_to.rel`, `why`, evidence text, or source names that
would reveal the recorded target action.

The previous grouped V2 training attempt was stopped after this issue was
identified. V3 fixed the reporting path by dropping non-visible refs. V4 made
writer candidates visible and the strict split dropped zero rows. V5 adds
structural candidate details and closes the remaining writer-context-read ref
selection misses without exposing final writer relations. V6 is the preferred
validation claim because it repeats the candidate-detail setup with a larger
explicit holdout of task ids `80` through `99`.

Explicit holdout split:

```bash
python scripts/operator/prepare_operator_sft_dataset.py \
  --trajectories /tmp/kernel-operator-trajectories-100-with-writer-candidate-details/trajectories.jsonl \
  --output /tmp/kernel-operator-sft-100-with-writer-by-task-anon-visible-candidate-details-holdout20 \
  --split-mode group \
  --group-key task_id \
  --eval-group-values-file /tmp/kernel-operator-holdout20-groups.txt \
  --anonymize-refs \
  --require-visible-target-refs \
  --force
```

Outputs:

- `train.jsonl`
- `eval.jsonl`
- `all.jsonl`
- `train_trajectories.jsonl`
- `eval_trajectories.jsonl`
- `all_trajectories.jsonl`
- `train_model_trajectories.jsonl`
- `eval_model_trajectories.jsonl`
- `all_model_trajectories.jsonl`
- `dropped_non_visible_target_refs.jsonl`
- `summary.json`

The user prompt excludes target actions, observed outcomes, benchmark gold
answers, and hidden raw memory.

For strict anonymized datasets:

- `*_trajectories.jsonl` keeps original refs for audit;
- `*_model_trajectories.jsonl` keeps anonymized refs for evaluation;
- predictions from anonymized prompts must be evaluated against
  `eval_model_trajectories.jsonl`;
- local SFT training should use `openai_train.jsonl` and `openai_eval.jsonl`
  because those files contain only `messages`.

Prompt leak audit:

```bash
python scripts/operator/audit_operator_sft_no_gold.py \
  /tmp/kernel-operator-sft-100-with-writer-by-task-anon-visible-candidate-details/openai_train.jsonl \
  /tmp/kernel-operator-sft-100-with-writer-by-task-anon-visible-candidate-details/openai_eval.jsonl \
  --output /tmp/kernel-operator-sft-100-with-writer-by-task-anon-visible-candidate-details/no-gold-audit.json
```

Expected result: `finding_count` is `0`.

### LongMemEval Trajectories

LongMemEval uses a separate exporter. Do not route LongMemEval rows through the
MemoryArena exporter; both exporters emit the same
`kernel-operator-trajectory-v1` contract so downstream preparation can consume
them together.

```bash
cargo run -p underpass-operator-benchmark-adapters-cli --bin longmemeval_operator_trajectory_export -- \
  --run <longmemeval-run-dir> \
  --artifacts <longmemeval-adapter-artifacts-dir> \
  --output <longmemeval-operator-trajectories-dir> \
  --expected-run-id <run-id> \
  --force
```

For LongMemEval smart-writer runs, include writer context reads:

```bash
cargo run -p underpass-operator-benchmark-adapters-cli --bin longmemeval_operator_trajectory_export -- \
  --run <longmemeval-smart-writer-run-dir> \
  --artifacts <longmemeval-smart-writer-artifacts-dir> \
  --output <longmemeval-smart-writer-operator-trajectories-dir> \
  --expected-run-id <run-id> \
  --include-writer-reads \
  --force
```

Mixed MemoryArena + LongMemEval SFT data is prepared by passing multiple
trajectory files. Keep `--split-mode group --group-key task_id`: MemoryArena
groups by task id, LongMemEval groups by question id, and writer rows use the
same logical question id.

```bash
python scripts/operator/prepare_operator_sft_dataset.py \
  --trajectories <memoryarena-operator-trajectories>/trajectories.jsonl \
  --trajectories <longmemeval-operator-trajectories>/trajectories.jsonl \
  --trajectories <longmemeval-smart-writer-operator-trajectories>/trajectories.jsonl \
  --output <mixed-operator-sft-dir> \
  --split-mode group \
  --group-key task_id \
  --anonymize-refs \
  --require-visible-target-refs \
  --force
```

## 2. Train LoRA

Strict next run:

```bash
python scripts/operator/train_operator_sft_lora.py \
  --train-jsonl /tmp/kernel-operator-sft-100-with-writer-by-task-anon-visible-candidate-details/openai_train.jsonl \
  --eval-jsonl /tmp/kernel-operator-sft-100-with-writer-by-task-anon-visible-candidate-details/openai_eval.jsonl \
  --model-id Qwen/Qwen2.5-0.5B-Instruct \
  --output-dir /tmp/kernel-operator-qwen05-lora-v5 \
  --epochs 3 \
  --batch-size 2 \
  --grad-accum 8 \
  --max-length 2048 \
  --bf16
```

Use `--fp16` instead of `--bf16` if the GPU does not support bfloat16.

## 3. Predict

```bash
python scripts/operator/predict_operator_sft.py \
  --dataset-jsonl /tmp/kernel-operator-sft-100-with-writer-by-task-anon-visible-candidate-details/eval.jsonl \
  --model-id Qwen/Qwen2.5-0.5B-Instruct \
  --adapter /tmp/kernel-operator-qwen05-lora-v5 \
  --output /tmp/kernel-operator-qwen05-predictions-v5 \
  --batch-size 8 \
  --force
```

## 4. Evaluate

```bash
cargo run -p underpass-operator-evaluation-cli --bin underpass_operator_policy_eval -- \
  --model-facing-eval /tmp/kernel-operator-sft-100-with-writer-by-task-anon-visible-candidate-details/eval.jsonl \
  --predictions /tmp/kernel-operator-qwen05-predictions-v5/predictions.jsonl \
  --output /tmp/kernel-operator-qwen05-predictions-v5-policy-eval.json \
  --details-output /tmp/kernel-operator-qwen05-predictions-v5-policy-details.jsonl
```

For non-anonymized smoke datasets, `eval_model_trajectories.jsonl` and
`eval_trajectories.jsonl` are equivalent. For strict anonymized SFT datasets,
use `--model-facing-eval <sft-dir>/eval.jsonl` first. Do not compare
model-facing refs such as `about_0001` and `ref_0001` directly against raw
trajectory refs.

The Kubernetes prediction job may create `/tmp/kernel-operator-qwen05-predictions-v5`
as `nobody`. In that case, write `policy-eval.json` to a sibling path as shown
above.

The policy evaluator reports both global metrics and `by_mode_eval`. Treat the
mode-specific breakdown as the release gate. For example, `read` can pass while
`write_context_read` remains diagnostic; do not promote a mixed global score if
one mode is hiding another mode's failures.

Use policy details to compare whether a new dataset or model actually improves
the same frozen probe set:

```bash
python scripts/operator/compare_operator_policy_details.py \
  --baseline-details <baseline-policy-details>.jsonl \
  --candidate-details <candidate-policy-details>.jsonl \
  --output <candidate-vs-baseline-details>.jsonl \
  --summary-output <candidate-vs-baseline-summary>.json \
  --force
```

The comparison groups rows by `target_capability_key` and reports whether each
probe improved, regressed, stayed correct, or stayed as an unresolved gap.

The main comparison is against:

- deterministic baseline;
- OpenAI generalist baseline;
- small trained operator.

Observed V3 ref-safe run on 2026-05-11 with `--batch-size 8`:

| Predictor | Total | Exact | Tool | Ref | Scope | Stop | Invalid | Unbounded |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Oracle baseline | 464 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 0 | 0 |
| Deterministic baseline | 464 | 0.349 | 1.000 | 0.606 | 1.000 | 1.000 | 0 | 0 |
| Qwen 0.5B LoRA V3 | 464 | 0.996 | 1.000 | 0.995 | 1.000 | 1.000 | 0 | 0 |

The V3 run produced 464 predictions with zero parse failures. The only two
exact mismatches used the correct tool and bounded arguments but selected a
different visible `kernel_inspect` ref in writer-context-read steps.

The batched Kubernetes prediction job completed in 3m24s including dependency
installation, model load, and generation. The previous unbatched path took 16m
for the same 464 rows.

Observed V4 candidate-visible run on 2026-05-11 with `--batch-size 8`:

| Predictor | Total | Exact | Tool | Ref | Scope | Stop | Invalid | Unbounded |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Oracle baseline | 615 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 0 | 0 |
| Deterministic baseline | 615 | 0.263 | 1.000 | 0.434 | 1.000 | 1.000 | 0 | 0 |
| Qwen 0.5B LoRA V4 | 615 | 0.993 | 1.000 | 0.993 | 1.000 | 1.000 | 0 | 0 |

V4 trained on 5,109 rows and evaluated on 615 rows, grouped by task with
synthetic model-facing refs. Prediction produced 615 rows with zero parse
failures and completed in 5m14s including dependency installation and model
load. The four exact misses are all writer-context-read choices where the model
selected a different visible candidate ref with the correct tool, scope, and
bounded arguments.

V5 candidate-detail run:

```bash
python scripts/operator/prepare_operator_sft_dataset.py \
  --trajectories /tmp/kernel-operator-trajectories-100-with-writer-candidate-details/trajectories.jsonl \
  --output /tmp/kernel-operator-sft-100-with-writer-by-task-anon-visible-candidate-details \
  --split-mode group \
  --group-key task_id \
  --eval-ratio 0.1 \
  --seed 42 \
  --anonymize-refs \
  --require-visible-target-refs \
  --force
```

This dataset keeps the same 5,109 train rows and 615 eval rows as V4, with zero
dropped non-visible target refs. Use `kernel-operator-qwen05-lora-v5` and
`kernel-operator-qwen05-predict-v5` for the Kubernetes LoRA/prediction jobs.

Observed V5 candidate-detail run on 2026-05-11 with `--batch-size 8`:

| Predictor | Total | Exact | Tool | Ref | Scope | Stop | Invalid | Unbounded |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Oracle baseline | 615 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 0 | 0 |
| Deterministic baseline | 615 | 0.263 | 1.000 | 0.434 | 1.000 | 1.000 | 0 | 0 |
| Qwen 0.5B LoRA V5 | 615 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 0 | 0 |

V5 trained on 5,109 rows and evaluated on 615 rows, grouped by task with
synthetic model-facing refs. Prediction produced 615 rows with zero parse
failures and completed in 4m55s including dependency installation and model
load. The training job completed in 35m27s, with final `eval_loss` 0.00966 and
`eval_mean_token_accuracy` 0.9957.

V6 explicit-holdout run:

```bash
python scripts/operator/prepare_operator_sft_dataset.py \
  --trajectories /tmp/kernel-operator-trajectories-100-with-writer-candidate-details/trajectories.jsonl \
  --output /tmp/kernel-operator-sft-100-with-writer-by-task-anon-visible-candidate-details-holdout20 \
  --split-mode group \
  --group-key task_id \
  --eval-group-values-file /tmp/kernel-operator-holdout20-groups.txt \
  --anonymize-refs \
  --require-visible-target-refs \
  --force
```

The holdout file reserves task ids `80` through `99` for eval. The split
contains 4,600 train rows and 1,124 eval rows, with zero dropped non-visible
target refs.

Observed V6 explicit-holdout strict rerun on 2026-05-14 with `--batch-size 8`:

| Predictor | Total | Exact | Tool | Ref | Scope | Stop | Invalid | Unbounded |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Oracle baseline | 1,124 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 0 | 0 |
| Deterministic baseline | 1,124 | 0.263 | 1.000 | 0.434 | 1.000 | 1.000 | 0 | 0 |
| Qwen 0.5B LoRA V6 holdout20 | 1,124 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 0 | 0 |

V6 trained on 4,600 rows and evaluated on 1,124 rows. Prediction produced
1,124 rows with zero failures under
`kernel-operator-action-contract-v1` and
`strict-no-additional-properties`. The training job completed in 33m01s, with
final `eval_loss` 0.01425 and `eval_mean_token_accuracy` 0.9954.

## 5. De-Anonymize Predictions For Raw Replay

Predictions from strict anonymized datasets contain synthetic refs such as
`ref_0001`. They are correct for offline model evaluation, but they cannot be
executed against a live kernel until those refs are mapped back to raw kernel
refs.

Scoped predictions may also contain synthetic about ids such as `about_0001`.
The de-anonymizer maps both refs and about ids from the paired model/raw
trajectory files. If a synthetic value was not visible in the paired trajectory,
the row fails fast instead of inventing a mapping.

Use the paired raw/model trajectory files to create evaluator-compatible raw
predictions:

```bash
python scripts/operator/deanonymize_operator_predictions.py \
  --raw-trajectories /tmp/kernel-operator-sft-100-with-writer-by-task-anon-visible-candidate-details-holdout20/eval_trajectories.jsonl \
  --model-trajectories /tmp/kernel-operator-sft-100-with-writer-by-task-anon-visible-candidate-details-holdout20/eval_model_trajectories.jsonl \
  --predictions /tmp/kernel-operator-qwen05-predictions-v6-holdout20-strict-20260514/predictions.jsonl \
  --output /tmp/kernel-operator-qwen05-predictions-v6-holdout20-strict-20260514-raw \
  --force
```

Outputs:

- `predictions.jsonl`: raw-ref predictions accepted by
  `underpass_operator_policy_eval`;
- `audit.jsonl`: one row per prediction with model action, raw action, and
  mapped synthetic refs and about ids;
- `failures.jsonl`: missing or unmappable refs;
- `summary.json`: selected/written/failure counts plus mapped ref/about totals.

Fail-fast behavior is intentional. If a predicted synthetic ref is not visible
in the paired model/raw trajectory, the row is rejected instead of inventing a
mapping.

Raw-ref evaluation:

```bash
cargo run -p underpass-operator-evaluation-cli --bin underpass_operator_policy_eval -- \
  --trajectories /tmp/kernel-operator-sft-100-with-writer-by-task-anon-visible-candidate-details-holdout20/eval_trajectories.jsonl \
  --predictions /tmp/kernel-operator-qwen05-predictions-v6-holdout20-strict-20260514-raw/predictions.jsonl \
  --output /tmp/kernel-operator-qwen05-predictions-v6-holdout20-strict-20260514-raw-policy-eval.json
```

Observed V6 strict de-anonymization result on 2026-05-14:

| Item | Value |
| --- | ---: |
| Selected predictions | 1,124 |
| Written raw predictions | 1,124 |
| Failures | 0 |
| Mapped synthetic refs | 5,240 |

Raw-ref policy eval stayed exact:

| Predictor | Total | Exact | Tool | Ref | Scope | Stop | Invalid | Unbounded |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Qwen 0.5B LoRA V6 holdout20, de-anonymized | 1,124 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 0 | 0 |

## 6. Replay Raw Predictions Against Live MCP

Raw-ref policy eval proves the predicted action matches the audited target
action. Live replay proves the predicted action is executable against the
kernel through the real MCP adapter and typed gRPC service.

Use `underpass_operator_mcp_replay` after de-anonymization:

```bash
cargo run -p underpass-operator-replay-cli --bin underpass_operator_mcp_replay -- \
  --trajectories /tmp/kernel-operator-sft-100-with-writer-by-task-anon-visible-candidate-details-holdout20/eval_trajectories.jsonl \
  --predictions /tmp/kernel-operator-qwen05-predictions-v6-holdout20-strict-20260514-raw/predictions.jsonl \
  --output /tmp/kernel-operator-qwen05-predictions-v6-holdout20-strict-20260514-mcp-replay-100 \
  --endpoint https://rehydration-kernel.underpassai.com \
  --limit 100 \
  --log-progress-every 25 \
  --force
```

Outputs:

- `results.jsonl`: one row per trajectory step with action, tool result,
  observed refs, missing expected refs, and extra observed refs;
- `summary.json`: selected rows, tool calls, stop actions, boundedness failures,
  MCP failures, ref coverage, action mix, and latency by action.

For long runs, `--log-progress-every N` writes compact JSONL progress events to
stderr without changing the replay result files.

The replay fails fast when:

- a prediction is missing;
- a prediction is malformed;
- a tool call is unbounded;
- MCP/gRPC returns an error;
- a tool call does not return the refs observed in the audited trajectory.

Observed 100-step strict live smoke on 2026-05-14:

| Item | Value |
| --- | ---: |
| Selected trajectory steps | 100 |
| Executed tool calls | 85 |
| Stop actions | 15 |
| Successful tool calls | 85 |
| Failed tool calls | 0 |
| Missing expected ref rows | 0 |
| Missing predictions | 0 |
| Invalid predictions | 0 |
| Unbounded tool calls | 0 |
| Partial result rows | 36 |
| Elapsed | 1m26.5s |

Observed full V6 holdout20 strict live replay on 2026-05-14:

| Item | Value |
| --- | ---: |
| Selected trajectory steps | 1,124 |
| Executed tool calls | 976 |
| Stop actions | 148 |
| Successful tool calls | 976 |
| Failed tool calls | 0 |
| Missing expected ref rows | 0 |
| Missing predictions | 0 |
| Invalid predictions | 0 |
| Unbounded tool calls | 0 |
| Extra observed ref rows | 848 |
| Extra observed refs | 7,216 |
| Partial result rows | 424 |
| Elapsed | 10m15.9s |

Extra observed refs mean the live kernel returned additional valid context
beyond the audited minimum. The replay fails only when expected refs are
missing.

Full strict replay action latency against the public TLS endpoint:

| Action | Count | Avg ms | Max ms |
| --- | ---: | ---: | ---: |
| `kernel_near` | 424 | 1,302.7 | 2,517 |
| `kernel_inspect` | 424 | 110.0 | 198 |
| `kernel_trace` | 128 | 127.1 | 181 |
| `stop` | 148 | 0.0 | 0 |

All partial results in the strict replay came from `kernel_near`. That is the
expected bounded behavior: the replay records `partial_result=true` and the page
object instead of accepting an unbounded traversal.

## 7. Next Scale Run

The validated claim today is the V6 explicit holdout20 run. The next publishable
operator cut should scale the same pipeline rather than changing model
semantics.

Run rules:

- top 1 gate: bounded pagination/progress/resume for remote audit and replay
  must be validated before using a run as publication evidence;
- start from a fresh audited MemoryArena smart-writer run;
- generate a fresh `run_id` for every live run or smoke;
- split by task id or run family, never by individual trajectory row;
- keep `--anonymize-refs` and `--require-visible-target-refs`;
- use raw refs only after prediction, through de-anonymization;
- run live MCP replay only after offline policy eval has zero invalid and
  unbounded actions.

Do not reuse the same `run_id` for a second live smoke. The deployed kernel is
append/projection based; previous writes under the same `about` can make early
asks observe answer feedback from an earlier attempt and create false
future-leak failures.

Recommended sequence:

First validate the P1.11.0 audit/replay pagination gate. The audit command must
emit progress by about/task and support resume before it is used as publication
evidence.

`memoryarena_kmp_run_audit` supports paged remote inspect through `--limit` and
`--offset`. It writes `inspect.next_offset` in the summary and emits JSONL
progress events to stderr and, optionally, `--progress-output`.

Temporal reads are also page-aware. `kernel_goto`, `kernel_near`,
`kernel_rewind`, `kernel_forward`, and `kernel_trace` expose a `page` object in
MCP structured output. Live replay writes that page into `results.jsonl`,
marks rows with `partial_result=true` when `page.has_more=true`, and reports
partial-result counts in `summary.json`.

```bash
cargo run -p rehydration-testkit --bin memoryarena_kmp_run_audit -- \
  --run <memoryarena-run-dir> \
  --endpoint <public-kernel-url> \
  --inspect \
  --expected-run-id <run-id> \
  --output <audit.json> \
  --limit 100 \
  --offset 0 \
  --log-progress-every 25 \
  --progress-output <audit-progress.jsonl> \
  --force

# For the next audit page, use inspect.next_offset from <audit.json> or the
# last progress event's next_offset as the new --offset.

cargo run -p underpass-operator-benchmark-adapters-cli --bin memoryarena_operator_trajectory_export -- \
  --run <memoryarena-run-dir> \
  --output <operator-trajectories-dir> \
  --include-writer-reads \
  --force

python scripts/operator/prepare_operator_sft_dataset.py \
  --trajectories <operator-trajectories-dir>/trajectories.jsonl \
  --output <operator-sft-dir> \
  --split-mode group \
  --group-key task_id \
  --anonymize-refs \
  --require-visible-target-refs \
  --force

python scripts/operator/train_operator_sft_lora.py \
  --train-jsonl <operator-sft-dir>/openai_train.jsonl \
  --eval-jsonl <operator-sft-dir>/openai_eval.jsonl \
  --model-id Qwen/Qwen2.5-0.5B-Instruct \
  --output-dir <operator-lora-dir> \
  --epochs 3 \
  --batch-size 2 \
  --grad-accum 8 \
  --max-length 2048 \
  --bf16

python scripts/operator/predict_operator_sft.py \
  --dataset-jsonl <operator-sft-dir>/eval.jsonl \
  --model-id Qwen/Qwen2.5-0.5B-Instruct \
  --adapter <operator-lora-dir> \
  --output <operator-predictions-dir> \
  --batch-size 8 \
  --force

cargo run -p underpass-operator-evaluation-cli --bin underpass_operator_policy_eval -- \
  --model-facing-eval <operator-sft-dir>/eval.jsonl \
  --predictions <operator-predictions-dir>/predictions.jsonl \
  --output <operator-policy-eval>.json
```

Only after that passes, de-anonymize and replay against live MCP as shown in
sections 5 and 6. Use `--limit 100` first; run the full replay only if the
smoke has zero missing predictions, invalid predictions, unbounded calls, MCP
failures, and missing expected refs.

## 8. Publication Packaging

Do not publish a model only from local accuracy. Package the release after the
P1.11 gate is clean:

- copy the model card template from
  `docs/product/huggingface/kernel-tool-operator-small-model-card-template.md`;
- copy the dataset card template from
  `docs/product/huggingface/kernel-operator-trajectories-dataset-card-template.md`;
- fill the release evaluation summary from
  `docs/product/huggingface/operator-release-eval-summary-template.md`;
- keep Hugging Face repos private first;
- verify download, local inference, offline eval, de-anonymization, and live MCP
  replay from the published artifacts;
- make the repos public only after that verification is clean.
