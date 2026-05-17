# Operator Dataset Quality Contract

Status: active contract
Date: 2026-05-16

This document defines what an Operator dataset must prove before it is used for
training.

The immediate lesson is simple:

```text
coverage is not learnability
row count is not diversity
observed behavior is not always an oracle
low eval loss is not policy quality
```

An Operator dataset is product infrastructure. If the dataset is weak, the
model can look healthy while learning the wrong policy.

The Rust Operator boundary is
[`operator-test-architecture.md`](operator-test-architecture.md). Dataset
validation must move toward the `underpass-operator-*` hexagonal crates:
domain rules, application use cases, contract DTOs, and infra mappers/adapters.
It must not live in duplicated Python validators or large benchmark binaries.

## Scope

This contract applies to datasets used to train or evaluate the small Operator
model that chooses KMP/MCP actions.

It covers:

- `operator-read`;
- `writer-pre-read`;
- prepared write execution;
- future bounded KMP/MCP operation profiles.

It does not train the model to invent memory semantics. Rich relation meaning,
`why`, evidence interpretation, money/date/counting operations, and domain
reasoning belong to a strong teacher, reader, plugin, or human writer.

Operator learns how to operate KMP. It does not become the writer.

## Dataset Row Contract

Every row is one decision step.

Required model-visible fields:

- `about`;
- `mode`;
- `goal`;
- `allowed_tools`;
- `visible_state`;
- budget or limit information when the tool supports it;
- refs and cursors that the model is allowed to use;
- enough page/partial metadata to decide whether to continue traversal.
- any requested bounds that the target action is expected to copy exactly.

Required non-model-visible fields:

- `target_action`;
- label source;
- provenance;
- observed outcome when the row came from a real run;
- quality audit metadata;
- original refs before anonymization, stored outside the model prompt when
  needed for replay.

Forbidden in model-facing prompts:

- `target_action`;
- benchmark answers;
- `has_answer`;
- `answer_session_ids`;
- expected refs;
- post-hoc tool results that were not visible at the decision time;
- raw secrets, credentials, or private prompts;
- hidden writer labels;
- any field that makes the answer visible without using KMP state.

## Hard Gates Before Training

Training must not start unless every gate below is explicitly recorded.

| Gate | Required Result |
| --- | --- |
| no gold leakage | 0 findings |
| shared action validator | every target action passes the same validator used for predictions |
| allowed tools | every target and prediction uses only tools listed in the row `allowed_tools` |
| visible refs | every target ref/cursor is visible in the row |
| bounded calls | every tool call has explicit budget/limit/window/page where applicable; `kernel_wake` and `kernel_ask` must carry `budget.tokens` in the safe Operator profile |
| fail-fast unsupported shape | unsupported dataset shapes fail before training |
| train/eval model-row overlap | 0 identical model-facing rows across train and eval |
| train/eval full-row overlap | 0 identical full rows across train and eval |
| same visible input with different labels | 0 unless explicitly a different goal/mode |
| prompt/tool parity | model prompt lists only action shapes allowed by the active profile |
| evaluation target namespace | predictions are evaluated against the same model-facing namespace, or explicitly de-anonymized before raw trajectory eval |
| label provenance | every row says deterministic, teacher, benchmark-derived, observed, or human |
| action distribution | recorded for train and eval |
| unique rows per action | recorded for train and eval |
| majority baseline | recorded before training |
| contrastive families | recorded before training |

If any hard gate fails, the dataset is not trainable. Keep the artifact for
forensics and create a new dataset cut.

Do not compare anonymized model-facing predictions directly against raw
trajectory ids. That makes correct actions look wrong and hides the real
failure mode. Use `underpass_operator_policy_eval --model-facing-eval` for SFT
eval rows, or de-anonymize predictions explicitly before raw trajectory eval.

Do not hide a deterministic copy decision inside the label. If the expected
action must copy a specific `limit`, `window`, `page`, `budget`, or scope, that
value must be visible in the row. If the model is supposed to choose those
values, the dataset must state and cover the sizing policy as a learnable
boundary.

## Learnability Gates

A dataset can pass contract coverage and still be bad for learning. Before
training, audit whether the model can realistically learn the policy boundary.

Required metrics:

- total rows;
- unique model-facing rows;
- unique model-facing rows per target action;
- unique groups per target action;
- duplicate model-row hashes;
- maximum duplicate count per model-row hash;
- train/eval overlap by model-row hash;
- action distribution in train and eval;
- target capability distribution in train and eval;
- label source distribution;
- task family distribution;
- majority-action baseline accuracy;
- per-action majority baseline;
- number of contrastive families;
- examples per contrastive family.

Rows that differ only by anonymized ref ids do not count as meaningful
diversity. They can be useful for replay, but they should not be treated as new
policy examples.

## Minimum Dataset Tiers

The numbers below are gates for the declared profile. A larger dataset can
still fail if most rows collapse into a few repeated states.

### Smoke Dataset

Use only for parser, validator, and CI checks.

| Requirement | Minimum |
| --- | ---: |
| profile capability coverage | 100% |
| train/eval overlap | 0 |
| unique examples per target action | 2 |
| contrastive families | 1 |

A smoke dataset must not be used to claim model quality.

### Diagnostic Training Dataset

Use to learn whether the recipe has a chance.

| Requirement | Minimum |
| --- | ---: |
| profile capability coverage in train | 100% |
| profile capability coverage in eval | 100% |
| train/eval model-row overlap | 0 |
| unique train examples per common action | 25 |
| unique eval examples per common action | 10 |
| unique train examples per rare action | 10 |
| unique eval examples per rare action | 5 |
| contrastive families per action boundary | 3 |
| maximum duplicate model-row count | 3 |
| majority-action baseline recorded | yes |

If a rare action cannot reach these counts, the run may be documented as a
negative probe, but it must not be treated as a proper training attempt.

### Candidate Training Dataset

Use for a candidate model or publication path.

| Requirement | Minimum |
| --- | ---: |
| unique train examples per common action | 50 |
| unique eval examples per common action | 20 |
| unique train examples per rare action | 20 |
| unique eval examples per rare action | 10 |
| contrastive families per action boundary | 8 |
| maximum duplicate model-row count | 2 |
| balanced eval split | required |
| natural distribution eval split | required |
| live replay-ready raw refs | required |

Training balance and evaluation realism are separate concerns. The training cut
may be balanced to teach the policy. The natural eval cut measures how it
behaves on real traffic. Both must exist for a serious claim.

## Writer-Pre-Read Required Boundaries

The `writer-pre-read` profile is especially sensitive because it teaches the
model how to read before a writer commits memory.

It must include contrastive examples for these boundaries:

| Boundary | Correct Decision |
| --- | --- |
| candidate exists but no surrounding context has been read | `kernel_near` |
| near result surfaced a specific candidate that needs details | `kernel_inspect` |
| inspected evidence is sufficient for the writer to proceed | `stop` |
| relation path matters and endpoints are visible | `kernel_trace` |
| trace result is partial and page metadata says more exists | `kernel_trace` continuation |
| trace result is complete, evidence is sufficient, and tool budget is exhausted | `stop` |
| candidate pool is ambiguous | expand/read more, not blind stop |
| budget is nearly exhausted and evidence is still insufficient | stop/escalate according to supported action contract |
| visible state only supports an anemic relation | stop with insufficient rich proof, do not invent |

For writer pre-read, observed benchmark trajectories are not automatically gold.
They show what happened in a previous run. They become training labels only when
one of these is true:

- the action is mechanically correct by deterministic policy;
- a strong teacher audited the state and action;
- a human accepted the row as the intended policy;
- the row is explicitly marked as observed behavior and used only as weak
  signal.

This prevents the model from learning a previous runner's habits as if they
were the desired KMP policy.

## Writer-Exec Required Boundaries

The `writer-exec` profile starts after pre-read and semantic writing. It does
not decide meaning. It decides whether a visible prepared payload may be sent to
KMP/MCP.

It must include contrastive examples for these boundaries:

| Boundary | Correct Decision |
| --- | --- |
| complete `draft_write.prepared_arguments` is visible | emit `prepared_tool_call` for `kernel_write_memory` with source `draft_write.prepared_arguments` |
| complete canonical payload is visible | emit `prepared_tool_call` for `kernel_ingest` with source `canonical_payload` |
| prepared write payload is missing | `stop` |
| rich relation lacks read-context proof | `stop` |
| relation is vague or unsupported | `stop` |
| prepared payload `about` differs from top-level `about` | `stop` |
| canonical payload is incomplete | `stop` |
| idempotency key already completed | `stop` |
| write options are not strict | `stop` |

The prompt for this profile must expose only write tools and `stop`. Read tools
belong to `writer-pre-read`; mixing them here makes the operator boundary
ambiguous.

The model-facing target for successful writer execution is intentionally
compact:

```json
{"action":{"type":"prepared_tool_call","tool":"kernel_write_memory","source":"draft_write.prepared_arguments"}}
```

or:

```json
{"action":{"type":"prepared_tool_call","tool":"kernel_ingest","source":"canonical_payload"}}
```

`prepared_tool_call` is not an MCP/KMP tool. It is an Operator-side decision
that must be resolved by the deterministic prepared-payload executor before any
real KMP/MCP call is made. The executor copies the visible prepared payload
byte-for-byte into the final `kernel_write_memory` or `kernel_ingest` action and
then validates that final action against the KMP action contract.

Hard gates for prepared payload rows:

- the model-facing `prepared_tool_call` must validate against the same strict
  Operator action contract used by prediction;
- the referenced prepared payload must be visible in the model-facing user
  payload;
- the prepared payload must match `target_action.arguments` exactly after any
  anonymization;
- prediction for this profile must use `--resolve-prepared-payloads`;
- live replay must receive final KMP/MCP tool calls, never
  `prepared_tool_call`.

## Trace Cursor And Stop Evidence Gates

Trace pagination must use the real KMP cursor shape.

`kernel_trace.page.cursor` is valid only when it is the numeric string returned
by KMP as `Trace.next_cursor`. Synthetic labels such as `page:next`,
`trace:page:2`, or benchmark-local cursors are not trainable targets.

Stop actions must be scored as evidence-bearing actions, not as a bare action
type. A `stop` target is correct only when these fields match exactly:

- `answer_policy`;
- `final_refs`;
- `reason` shape.

A policy evaluator that gives stop credit without exact `final_refs` is not
strict enough for Operator training claims.

Write provenance must also use supported `source_kind` values only:

- `human`;
- `agent`;
- `projection`;
- `derived`.

Synthetic labels such as `synthetic_conformance` are dataset provenance, not
KMP/MCP write provenance.

All enum-like KMP/MCP fields used by Operator must be exact strings. Do not
trim, lowercase, or otherwise normalize:

- `answer_policy`;
- `budget.detail`;
- memory relation `rel`;
- memory relation `class`;
- memory relation `confidence`;
- memory provenance `source_kind`.

Public KMP/MCP inputs and Operator targets must use the canonical wire value.
If the canonical value is `contradicts`, the dataset must not teach
`conflicts`, `conflicts_with`, `CONTRADICTS`, or whitespace-wrapped variants.

## Anti-Collapse Checks

Before training, compute what a trivial model would get by always choosing the
most common action.

Examples of collapse patterns:

- always `kernel_inspect`;
- always `kernel_near`;
- never `kernel_trace`;
- never `stop`;
- correct JSON but wrong tool;
- correct tool but wrong cursor/ref;
- correct ref but wrong scope/window/page;
- broad traversal where a bounded inspect was required;
- inspect where a page continuation was required.

If the majority baseline is high, the dataset must include a balanced eval split
and enough contrastive rows to make policy learning visible.

The model must beat the majority baseline by a meaningful margin on exact action
accuracy and on per-action metrics. Aggregate accuracy alone is not enough.

## Synthetic Use-Case Coverage Is Mandatory

Real benchmark traces are not enough. They are useful for realism, latency,
noise, and natural distributions, but they do not guarantee that every KMP/MCP
case of use appears often enough for a small model to learn it.

Every serious Operator dataset must contain a synthetic conformance block with
enough samples for each supported case of use.

The shape is:

```text
synthetic conformance rows -> deliberate coverage and balance
real benchmark traces      -> realism and natural distribution
held-out synthetic rows    -> API/MCP contract regression
held-out real traces       -> real workload regression
```

Do not wait for a benchmark to accidentally cover the API. If Operator is
expected to use a KMP/MCP capability, the dataset must include synthetic
families for it.

### Use-Case Families

At minimum, synthetic data must cover the active profile across these families:

| Family | Examples |
| --- | --- |
| wake/init | `kernel_wake` from intent, bounded context budget |
| ask/evidence | `kernel_ask` with `evidence_or_unknown`, `show_conflicts`, `best_effort` |
| temporal near | `kernel_near` around a visible ref with bounded windows |
| temporal goto | `kernel_goto` to a visible ref |
| temporal rewind | `kernel_rewind` from a visible ref |
| temporal forward | `kernel_forward` from a visible ref |
| trace first page | `kernel_trace` with visible endpoints and first page |
| trace continuation | `kernel_trace` continuation when page metadata exposes numeric KMP `Trace.next_cursor` |
| inspect detail | `kernel_inspect` for a visible ref, raw disabled by default |
| dimension scope | `current_about`, explicit `abouts`, intentional `all_abouts` when supported |
| dimension exact scope ids | `dimensions.scope_ids` when exact dimension-scope filtering is needed |
| pagination/bounds | different legal `limit`, `window`, `budget`, and page shapes |
| budget detail | `budget.detail` values `compact`, `balanced`, and `full` when the profile is expected to choose detail tier |
| stop | stop when enough evidence has been gathered |
| anti-invention | stop/escalate when refs, relation proof, or scope are not visible |
| writer pre-read | read enough context before a writer commits a memory relation |
| prepared write | execute write only when complete payload and proof are visible |
| fail-fast invalid request | unsupported shape is rejected by generator/evaluator, not learned as a fuzzy action |

The exact enabled families depend on the profile. For example,
`writer-pre-read` does not need to train full prepared writes, but it does need
near, inspect, trace, stop, scope, page, and anti-invention cases.

Raw memory flags are security-sensitive. If a profile excludes
`kernel_inspect.include.raw=true` or temporal `include.raw_refs=true`, that
exclusion must be explicit in the dataset report. Do not count a safe-by-default
profile as complete raw-inspection API coverage.

For the current safe read/write profiles, those raw flags are not merely absent
from the dataset: they must fail validation. A future audit profile may allow
them, but it must have its own prompt, validator policy, dataset distribution,
and replay evidence.

The coverage report must make these exclusions visible, not implied. At minimum
it must expose:

- `target_answer_policies`;
- `target_budget_details`;
- `target_dimension_scope_ids`;
- `target_temporal_raw_refs`;
- `target_inspect_raw`.

Write coverage must also be visible. A prepared-write Operator profile is not
the same thing as full authoring coverage for every legal `kernel_ingest` or
`kernel_write_memory` shape. At minimum the report must expose:

- `target_write_memory_options`;
- `target_write_memory_dry_run`;
- `target_write_memory_strict`;
- `target_write_memory_idempotency_key`;
- `target_write_memory_read_context`;
- `target_write_memory_current_evidence`;
- `target_write_memory_source_kind`;
- `target_write_memory_relation_proof`;
- `target_ingest_dry_run`;
- `target_ingest_dimensions`;
- `target_ingest_relations`;
- `target_ingest_evidence`;
- `target_ingest_provenance`;
- `row_parse_failures`;
- `row_parse_failure_examples`;
- `target_action_contract_failures`;
- `target_action_contract_failure_examples`.

This prevents a dry-run-only, explicit-options, prepared-payload dataset from
being mistaken for commit-write or raw-ingest coverage.

`row_parse_failures` must be `0`. A row that cannot be parsed as a raw
trajectory or model-facing SFT row is not a skipped profile row; it is a broken
dataset.

`target_action_contract_failures` must be `0`. Coverage reporting is not allowed
to count an invalid action as useful coverage. Tool counters, capability
counters, and shape distributions are counted only after the target resolves to
an executable action and passes the strict Operator/KMP action contract.

For prepared writer actions, the wrapper is not enough. A
`prepared_tool_call` only counts when its `source` resolves to a visible
prepared payload, the payload belongs to the same `about`, and the resulting
`tool_call` is valid. Missing or mismatched prepared payloads are action
contract failures, not partial coverage. Dataset summaries must also distinguish
prepared calls by `tool` and `source`; a generic `prepared_tool_call` count is
too coarse to audit write versus ingest execution.

For the `stop` versus `kernel_trace` boundary, the model-facing row must expose
whether the last trace result was partial, whether more pages exist, and how
many tool calls remain. If the target is `stop` because the trace is complete,
that must be learnable from visible state, not from a hidden label.

### Minimum Synthetic Counts

Counts apply per active use-case family, after deduplication by model-facing
state.

| Tier | Train unique states | Eval unique states | Contrastive variants |
| --- | ---: | ---: | ---: |
| smoke | 2 | 2 | 1 |
| diagnostic | 20 | 10 | 3 |
| candidate | 50 | 20 | 8 |

For a decision boundary inside a use case, such as `near` versus `inspect` or
`trace first` versus `trace continue`, the dataset must include both sides of
the boundary. One side alone does not teach the policy.

Synthetic rows should not be created by copying the same template with only a
different ref id. They must vary the visible state that justifies the action:

- candidate count;
- candidate roles;
- prior observed refs;
- last tool;
- page/partial metadata;
- remaining budget;
- dimension scope;
- relation proof visibility;
- ambiguity level;
- whether enough evidence has already been read.

If a use-case family cannot reach the minimum count, the dataset verdict cannot
be `trainable`.

## Prompt Surface Rules

The model-facing system prompt must match the active profile.

Rules:

- the system prompt must list only tools supported by the active prompt
  profile;
- every raw trajectory and model-facing row must declare `allowed_tools`;
- `allowed_tools` items must be non-empty strings with no duplicates;
- `allowed_tools` must stay inside the row `mode` boundary:
  `read` may only expose read tools, `write_context_read` may only expose
  writer pre-read tools, and `write` may only expose write execution tools;
- each row-level target action must use only tools present in that row's
  `allowed_tools`;
- fail if raw `allowed_tools` contains tools outside the active prompt profile;
- list only action shapes supported by the active profile;
- avoid showing write actions in a read-only or read-before-write dataset unless
  the row can actually use them;
- keep field names identical to the strict action contract;
- keep `about` and node refs visually distinct after anonymization;
- do not rely on hidden comments or natural language hints to disambiguate the
  target action.

The trainer, predictor, policy evaluator, coverage reporter, MCP replay, and
LLM baseline must repeat the same gate before loading model dependencies or
executing tools. A dataset generated by a different script or edited by hand is
not accepted just because it is JSONL.

Kubernetes training and prediction jobs must run the same validate-only
preflight before installing dependencies, loading a model, or deleting an output
directory. Historical jobs whose dataset no longer passes the current contract
must be suspended and annotated with the reason.

This is enforced by:

```text
bash scripts/ci/check-operator-k8s-jobs.sh
```

The check keeps an explicit allowlist of current Operator jobs. Every other
`k8s/kernel-operator*.yaml` manifest must be suspended and must carry a
quarantine reason. Current jobs must run `--validate-only` before dependency
installation, output deletion, training, or adapter-backed prediction. Current
jobs must also read training/eval datasets from the preserved read-only
`/operator-artifacts` mount, not from `/tmp`, and must write adapters and
prediction outputs to durable `/operator-runs` storage.

Before launching any current Operator GPU job, run the current artifact gate:

```text
bash scripts/operator/check_current_operator_artifacts.sh
```

That gate verifies the recorded sha256 values for canonical and OpenAI-format
SFT files, runs validate-only over the actual OpenAI train/eval files consumed
by training jobs, validates prediction inputs, and checks profile coverage over
train/eval rows.

A prompt that contains many unavailable actions can make a small model learn
surface imitation instead of the intended policy.

## Required Dataset Report

Every serious dataset cut must produce a short report before training starts.

Minimum report:

```text
dataset_id:
source_paths:
label_sources:
rows_total:
rows_train:
rows_eval:
unique_model_rows_total:
unique_model_rows_train:
unique_model_rows_eval:
train_eval_model_row_overlap:
full_row_overlap:
max_duplicate_model_row_count:
action_distribution_train:
action_distribution_eval:
unique_model_rows_by_action_train:
unique_model_rows_by_action_eval:
capability_coverage_train:
capability_coverage_eval:
api_mcp_contract_profile:
api_mcp_profile_contract_coverage:
api_mcp_target_capability_coverage:
answer_policy_distribution:
budget_detail_distribution:
raw_access_policy:
dimension_scope_ids_distribution:
write_memory_options_distribution:
write_memory_dry_run_distribution:
write_memory_strict_distribution:
write_memory_idempotency_key_distribution:
write_memory_read_context_distribution:
write_memory_current_evidence_distribution:
write_memory_source_kind_distribution:
write_memory_relation_proof_distribution:
ingest_dry_run_distribution:
ingest_dimensions_distribution:
ingest_relations_distribution:
ingest_evidence_distribution:
ingest_provenance_distribution:
row_parse_failures:
row_parse_failure_examples:
target_action_contract_failures:
target_action_contract_failure_examples:
contrastive_families:
synthetic_use_case_families:
synthetic_use_case_min_counts:
synthetic_rows_by_family:
real_rows_by_family:
majority_action_baseline:
prompt_tool_parity:
model_facing_target_projection:
prepared_payload_resolution_required:
trace_page_cursor_shape:
stop_evidence_gate:
source_kind_validation:
validated_by_train_preflight:
validated_by_prediction_preflight:
verdict:
reason:
```

The verdict must be one of:

| Verdict | Meaning |
| --- | --- |
| `trainable` | Passes contract and learnability gates for the declared tier. |
| `diagnostic-only` | Safe to run as an experiment, but not strong enough for claims. |
| `smoke-only` | Useful only for parser/validator/CI. |
| `quarantine` | Do not train from it. |

## Example Rejection Pattern

This pattern must be rejected for real training:

```text
raw source rows: thousands
selected rows: hundreds
unique model-facing rows: a few dozen
rare target actions: one or two examples
dominant action: near or inspect
train/eval overlap: fixed
contract coverage: 100%
```

That dataset can pass leakage and coverage checks while still teaching the
model to collapse into the dominant action.

The fix is not more epochs. The fix is a better dataset:

- more unique states;
- more contrastive examples;
- stronger label provenance;
- balanced training cut;
- balanced eval plus natural eval;
- per-action gates;
- policy eval before any live replay claim.

## Stop Rule

If the pre-training dataset report says the dataset is only `diagnostic-only`,
the run must say so before GPU is used.

If the report says `smoke-only` or `quarantine`, do not train.

If training still happens for investigation, the resulting model must be marked
`failed`, `baseline-only`, or `internal-only`. It cannot be promoted.
