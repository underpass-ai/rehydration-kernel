# 2026-05-17 KMP Cursor Contract Refresh

Status: dataset-ready
Date: 2026-05-17

## 1. Purpose

Close the gaps found while auditing the current Operator API/MCP training
contract after strict KMP cursor, enum, prepared-payload, and source-kind
validation became mandatory.

This is not a model result. It is a contract/data/manifests refresh.

## 2. Gaps Found

| Gap | Impact | Fix |
| --- | --- | --- |
| `writer-pre-read` coverage reporter used the full read tool list | Could overstate profile/API coverage | Reporter now uses `operator_allowed_writer_pre_read_tools()` |
| coverage reporter only understood raw trajectories | Final SFT JSONL rows could report 0% or misleading coverage | Reporter now reads model-facing SFT `messages` |
| `prepared_tool_call` was not resolved for coverage | Write datasets looked like they did not cover `kernel_write_memory` / `kernel_ingest` | Reporter resolves visible prepared payloads for coverage |
| no write-only coverage profile | `full` incorrectly required read tools for write-exec datasets | Added `--profile write` |
| current support jobs installed dependencies before preflight | Bad datasets could burn GPU time | Active jobs now run validate-only first |
| standalone writer-exec prepared-v1 used invalid `source_kind=synthetic_conformance` | Strict resolved payload validation failed | Old jobs quarantined; source-kind-agent dataset/manifests are the valid path |
| standalone read-api-mcp-v1 and writer-pre-read-v4 used symbolic trace cursors | Strict `Trace.next_cursor` contract failed | Regenerated KMP-cursor SFT datasets |
| historical operator manifests were not suspended | Easy to relaunch obsolete experiments accidentally | All non-current `kernel-operator*.yaml` jobs are suspended with quarantine annotations |
| Kubernetes job policy was manual | A later edit could silently reactivate obsolete jobs or move preflight after dependency/model work | Added `scripts/ci/check-operator-k8s-jobs.sh` and wired it into `quality-gate.sh` |
| current Kubernetes jobs still read datasets from `/tmp` | A tmp cleanup could make a valid run impossible or accidentally pick up stale regenerated data | Current jobs now mount the preserved artifact root read-only at `/operator-artifacts` and read train/eval data from there |
| current Kubernetes jobs still wrote adapters/predictions to `/tmp` | A valid training run could lose its adapter before prediction or publication | Current jobs now write final adapters and predictions to durable `/operator-runs` storage |
| current Kubernetes jobs had no per-manifest pairing check | A valid-looking predictor could point at the wrong dataset or adapter | `check-operator-k8s-jobs.sh` now checks expected dataset, train output, prediction output, and adapter path for every current manifest |
| GitHub Actions did not execute the new Operator job policy | PR checks could miss a reactivated historical job | Added an explicit `operator-k8s-policy` job to `.github/workflows/quality-gate.yml` |
| coverage over mixed datasets was not profile-filtered | `writer-pre-read` coverage could count write rows, and `write` coverage could count pre-read rows | Reporter now filters rows by `mode` before counting profile coverage |
| mixed coverage reports hid how many rows were filtered | A 100% result was harder to audit by eye | Reporter now emits `rows_total`, `rows_included`, and `rows_skipped_by_profile` |
| current artifact coverage required many manual commands | Easy to validate only eval, skip train, or miss the mixed writer profile | Added `scripts/operator/check_current_operator_artifacts.sh` for train/eval coverage over all current profiles |
| artifact gate did not check the files used by SFT training | `openai_train.jsonl` / `openai_eval.jsonl` could drift while canonical `train.jsonl` / `eval.jsonl` still passed | Artifact gate now hashes and validate-only checks OpenAI-format train/eval files |
| preserved artifacts had no recorded hashes | A later run could not prove which exact dataset files were used | Added sha256 hashes for critical SFT summaries, train/eval JSONL, and orchestration oracle eval |
| recorded hashes were not enforced | A modified artifact could still pass coverage if shape stayed valid | `scripts/operator/check_current_operator_artifacts.sh` now verifies the recorded sha256 values before coverage |
| valid regenerated data lived only under `/tmp` | Artifacts could disappear | Copied current valid artifacts to the parent artifacts directory |
| Kubernetes mount policy checked loose lines instead of the concrete mount blocks | A manifest could contain `readOnly: true` somewhere else and still pass | `check-operator-k8s-jobs.sh` now validates the `operator-artifacts` and `operator-runs` mount/hostPath blocks explicitly |
| Python action validator accepted some invalid `kernel_ingest` shapes | Offline prediction/eval could accept a write that real MCP/gRPC would reject | `predict_operator_sft.py` now requires entry `coordinates`, validates metadata as string maps, supports coordinate `rank`/`ingested_at`/`valid_until`, and handles optional evidence `supports` |
| coverage report hid `answer_policy` distribution | A profile could show 100% coverage while missing an enum value such as `best_effort` | Coverage now reports `target_answer_policies` |
| coverage report hid `budget.detail`, `dimensions.scope_ids`, and raw-access distribution | A profile could look complete while never teaching detail tiers, exact dimension-scope filtering, or raw-access policy | Coverage now reports `target_budget_details`, `target_dimension_scope_ids`, `target_temporal_raw_refs`, and `target_inspect_raw` |
| `kernel_ingest` dry-run rejected incremental appends with `memory.dimensions: []` | MCP dry-run and gRPC commit did not accept the same valid API shape | MCP schema, dry-run planning, Python validation, and Rust Operator validation now allow empty dimensions for incremental append while still requiring entry coordinates |
| Rust and Python Operator validators disagreed with the KMP ingest schema for optional fields | The same prepared payload could pass one gate and fail another, or the Operator could be trained against a stricter shape than the API | Both validators now align on optional provenance, optional relations/evidence, optional evidence `supports`, metadata string maps, and non-structural relation proof rules |
| contract coverage was easy to read as exhaustive MCP schema coverage | A 100% profile score could be misinterpreted as every optional field/enum/raw mode in every MCP tool | The run document now scopes the result to selected Operator capabilities and lists uncovered schema areas explicitly |
| coverage report hid write-mode safety and optional payload shape | A dry-run prepared-write profile could be mistaken for commit-write or raw-ingest coverage | Coverage now reports `kernel_write_memory` options/dry-run/strict/idempotency/read-context/current-evidence/source-kind distributions and `kernel_ingest` dry-run/dimensions/relations/evidence/provenance distributions |
| `incomplete_canonical_payload` synthetic stop row became valid after widening ingest optional fields | Future generated data could teach `stop` for a canonical payload that KMP would accept | The invalid fixture now removes required entry coordinates instead of relying on optional `evidence` emptiness |
| safe read profile said raw access was excluded, but temporal `include.raw_refs=true` still passed validators | A model could request temporal raw audit refs and still pass the strict action contract | Rust and Python Operator validators now reject temporal `raw_refs=true`; raw access requires a separate audited profile |
| structural `kernel_write_memory` relations could omit proof, but the compiled ingest preview still emitted empty evidence | A valid structural relation could become an invalid canonical ingest payload or be rejected by stricter Operator validators | `kernel_write_memory` now omits empty `why`/`evidence` fields and empty relation evidence items for structural relations; Operator validators accept structural links without proof while non-structural links still require proof |
| coverage reporter counted target actions without validating their final action contract | Invalid actions could still make a capability look covered if only the coverage report was run | `underpass_operator_contract_coverage` now reports `target_action_contract_failures`, keeps examples, and fails fast when any included target action is invalid |
| coverage reporter counted before validating | An invalid target could still populate tool/distribution counters in the JSON report before the fail-fast error | Coverage is now counted only after the target action resolves to an executable `tool_call` and passes the strict action contract |
| malformed SFT rows were indistinguishable from profile exclusions | Broken model-facing rows could be hidden as skipped rows when measuring a mixed dataset | `row_parse_failures` is reported separately and fails fast; skipped rows are only rows intentionally outside the selected profile |
| coverage/eval parsers accepted looser chat rows than train/predict | A row could pass coverage or policy evaluation with extra/misordered messages but fail during training or prediction | Coverage and policy evaluation now require the same exact `system/user/assistant` three-message shape as the trainer and predictor |
| rows without explicit Operator `mode` defaulted into read coverage | Mixed datasets could accidentally count unclassified raw rows as read-profile evidence | Coverage now treats missing/unsupported `mode` as `row_parse_failures`, not profile skips |
| prepared actions could cover `prepared.source:*` without resolving payload | A prepared writer dataset could look covered even if the visible payload was missing or mismatched | `prepared_tool_call` coverage now requires a resolvable visible payload that validates as the final KMP tool call |
| SFT preparation matched prepared payloads without validating the resolved final call | Bad prepared writer data could be written and only fail later in trainer/predictor validation | `prepare_operator_sft_dataset.py` now validates the resolved `tool_call` during dataset generation |
| writer prediction/eval could forget prepared-payload resolution | Metrics or replay inputs could contain `prepared_tool_call` placeholders instead of executable KMP calls | Predictor and policy evaluator now fail fast when prepared targets are used without `--resolve-prepared-payloads` |
| valid KMP actions were not checked against row `allowed_tools` during prediction/eval | A model could call a valid but disallowed tool and be counted as merely tool-incorrect instead of invalid | Predictor and policy evaluator now mark predictions invalid when the predicted tool is outside the row allow-list |
| coverage did not enforce row `allowed_tools` | A valid but disallowed target could still mark a tool capability as covered | Coverage now rejects disallowed target tools before counting capabilities or distributions |
| raw eval/coverage rows could omit `allowed_tools` | A raw trajectory could be scored without proving the tool boundary visible to the Operator | Raw rows now require `allowed_tools`; missing or malformed values are parse failures, not skipped rows |
| `allowed_tools` could contain tools outside the row mode | A `read` row could advertise write tools, or a `write` row could advertise navigation tools, weakening API/MCP parity | The shared Rust testkit mode map and Python gates now reject out-of-mode tools in preparation, training, prediction, policy eval, coverage, MCP replay, and the LLM baseline |
| MCP replay could execute a prediction outside row `allowed_tools` | A live replay could call a valid KMP tool that was not available in the original Operator prompt | MCP replay now reads and validates trajectory `allowed_tools` and fails disallowed predictions before tool execution |
| FunctionGemma native path had a looser `allowed_tools` gate | The legacy read-only path could accept rows that the SFT path would reject | FunctionGemma validation now requires clean `allowed_tools`, checks mode boundaries, and rejects targets outside the row allow-list |
| policy eval did not fail on invalid target actions | Metrics could be computed against a broken eval target | Policy eval now validates every resolved target action against the strict KMP Operator action contract before scoring |
| `kernel_wake` and `kernel_ask` could pass the Operator contract without explicit token budget | Safe-profile reads could rely on API defaults instead of teaching bounded calls | Rust and Python Operator validators now require `budget.tokens` for `kernel_wake` and `kernel_ask` |

## 2.1 Follow-up Gaps Not Closed In This Artifact Cut

These are not regressions in the preserved 2026-05-17 artifacts, but they limit
what can be claimed from them.

| Gap | Impact | Required follow-up |
| --- | --- | --- |
| `kernel_ask.answer_policy=best_effort` is supported by MCP/gRPC but absent from the current read SFT cut | The read profile coverage is 100% for the scoped profile, not for every `kernel_ask` enum value | Add `answer_policy:best_effort` capability, regenerate read/orchestration datasets, update hashes, and retrain/evaluate |
| `budget.detail` is supported by KMP/MCP but absent from the current Operator data | The model has learned bounded token/depth budgets, not detail-tier selection | Add `budget.detail:{compact,balanced,full}` synthetic cases and require the distribution in the dataset report |
| `dimensions.scope_ids` is supported by the API but absent from current read data | Current dimension coverage includes mode and about scope, but not exact dimension-scope id filtering | Add synthetic read cases for `scope_ids` across temporal and ask tools |
| `kernel_inspect.include.raw=true` and temporal `include.raw_refs=true` are intentionally not in the Operator data | The current Operator is safe-by-default and cannot request raw memory through learned policy | Keep this as an explicit security-scoped exclusion, or create a separate audited raw-inspection profile |
| writer-exec is a prepared-write profile, not a raw full `kernel_ingest` authoring profile | Operator can decide whether to execute visible prepared payloads; it is not trained to compose every legal ingest payload shape | Keep public wording scoped to prepared writer execution, or create a separate raw-ingest Operator profile |
| `kernel_ingest` commit mode is supported by KMP/MCP, but current Operator write-exec remains dry-run-bounded | The current model should not be claimed as trained for autonomous commit writes | Add a separate audited commit-write profile before allowing Operator to emit write commits |
| `kernel_ingest` API accepts more optional payload shape than the writer-exec prompt currently teaches | Valid incremental append, metadata, coordinate, optional provenance, and optional evidence variants are now validator-safe, but the current dataset does not teach all of them | Add synthetic canonical-ingest variants if Operator must cover raw ingest payload selection |
| preserved writer-exec artifacts still reflect the stricter prepared-payload shape used before the validator widening | The checked-in generator prompt is ready for future incremental append/reduced-payload cuts, but the current artifact hashes do not contain those examples | Regenerate writer-exec/writer-orchestration datasets when the next run intentionally teaches incremental append or reduced canonical payloads |

## 3. Current Valid Artifact Root

```text
../rehydration-kernel-artifacts/operator/2026-05-17-kmp-cursor-contract-refresh/
```

Contains:

```text
kernel-operator-conformance-read-api-mcp-v1-kmp-cursor-20260517
kernel-operator-conformance-writer-pre-read-v4-kmp-cursor-v2-20260517
kernel-operator-conformance-writer-exec-v1-kmp-cursor-v2-20260517
kernel-operator-sft-read-api-mcp-v1-kmp-cursor-20260517
kernel-operator-sft-writer-pre-read-v4-kmp-cursor-v2-20260517
kernel-operator-sft-writer-exec-v1-prepared-exec-source-kind-agent-20260517
kernel-operator-sft-writer-orchestration-v2-kmp-cursor-20260517
```

## 4. Dataset Gates

| Dataset | Rows | Train | Eval | Profile | Coverage | Preflight |
| --- | ---: | ---: | ---: | --- | --- | --- |
| `read-api-mcp-v1-kmp-cursor-20260517` | 716 | 501 | 215 | read | 24/24 | train/predict OK |
| `writer-pre-read-v4-kmp-cursor-v2-20260517` | 576 | 403 | 173 | writer-pre-read | 21/21 | train/predict OK |
| `writer-exec-v1-prepared-exec-source-kind-agent-20260517` | 234 | 164 | 70 | write | 9/9 | train/predict OK |
| `writer-orchestration-v2-kmp-cursor-20260517` | 810 | 567 | 243 | writer-pre-read + write | 21/21 + 9/9 | train/predict OK |

The read cut currently observes these answer policies:

```text
evidence_or_unknown
show_conflicts
```

It does not yet contain `best_effort`, even though KMP/MCP supports it. Treat
the `24/24` value above as scoped read-profile coverage, not complete
`kernel_ask` enum coverage.

The same read eval coverage now exposes these additional distributions:

```text
target_dimension_scope_ids: absent=156, present=0
target_budget_details: unspecified=179, compact=0, balanced=0, full=0
target_temporal_raw_refs: false=87, true=0
target_inspect_raw: false=18, true=0
```

Those zeros are intentional observations, not failures in this artifact cut.
They define the next synthetic data gaps.

The writer-exec eval coverage now exposes the current safety envelope:

```text
target_write_memory_options: present=23
target_write_memory_dry_run: true=23
target_write_memory_strict: true=23
target_write_memory_idempotency_key: present=23
target_write_memory_read_context: present=23
target_write_memory_current_evidence: present=23
target_write_memory_source_kind: present=23
target_write_memory_relation_proof: non_structural_complete=23
target_ingest_dry_run: true=7
target_ingest_dimensions: non_empty=7
target_ingest_relations: non_empty=7
target_ingest_evidence: non_empty=7
target_ingest_provenance: present=7
row_parse_failures: 0
target_action_contract_failures: 0
```

This confirms the current write dataset is explicit, strict, dry-run-only, and
complete-payload oriented. It is valid for prepared safe execution, not for
commit-write, omitted-defaults, or reduced incremental ingest coverage.

## 5. Artifact Hashes

Critical SFT artifact hashes:

| Dataset | File | sha256 |
| --- | --- | --- |
| read API/MCP | `summary.json` | `0e2974fcedb446f6f0e8428de1a43ff0f744fc68a26a9874705f30d146370d28` |
| read API/MCP | `train.jsonl` | `a663da76113fcd4a275a1f3af91ec20e65a9248651c3f56aa8705ad400ccaa71` |
| read API/MCP | `eval.jsonl` | `db71a840d96a5a142c577d6bb740c3bb09f5fdf16fb82f1119194f7daab54cc8` |
| read API/MCP | `openai_train.jsonl` | `d9a0f5fd5050b88ca12968e04ad469886c90db3125d31332572243d7440fa0f6` |
| read API/MCP | `openai_eval.jsonl` | `5e09edb27c0e7ba9c0c3776c676c6fbf04cc8620bca1ab4bc38a1f1ed577cd72` |
| writer pre-read | `summary.json` | `d57db2797eab07409cd83ab5974b55990c05304cc39320a4e348b0071099f7ce` |
| writer pre-read | `train.jsonl` | `87d7e41f1869af14332f22c42da814283d7a5b21026fede92520ffee75230491` |
| writer pre-read | `eval.jsonl` | `278d7d5bacd1f85f6f401b227d83ffb1fe3fb35fe08ab8216f0401e003da858d` |
| writer pre-read | `openai_train.jsonl` | `1ff100e1bb00ebc10ae0ccbc0e19a27d50d7f93375f1045fd8ac3f2148775bc6` |
| writer pre-read | `openai_eval.jsonl` | `395fdc1fdc87f3b70f5799c265a1d2baa97fe0f55673d2da09ebaaa1f3e73040` |
| writer exec prepared | `summary.json` | `6df2a058031d3d2ec1fb0f4ec88b5dfb25cc9d74265c452d7c1ac871281af498` |
| writer exec prepared | `train.jsonl` | `43cdd49f6f7b8b20ae79ea970a208678a2d8fefd02ea6ee96c20b934356abc76` |
| writer exec prepared | `eval.jsonl` | `162331bbcd256d9cfd837545c7d7bcdcd83cd75e7972e2d7fdce5c56f05c1180` |
| writer exec prepared | `openai_train.jsonl` | `5f80541e90d1fb3235b76385c913a2123e6dd6988b06e5cee0fa1084a2ea0dbe` |
| writer exec prepared | `openai_eval.jsonl` | `d2ee1c5cae3d5754b194aeaa1c984135452f824a56383f87fe89fb8c43f4f875` |
| writer orchestration v2 | `summary.json` | `4fe0f17c25a98a7157c2f28547a051baa41e0ae586fc68ae673d0ae83b8a3227` |
| writer orchestration v2 | `train.jsonl` | `3cc50ea0f076949e7b004b61df92171a9463ad32813f2c04d7a542dcf05e8d4f` |
| writer orchestration v2 | `eval.jsonl` | `61b4d8b3abdc1c64aba7b7d64804a1ee6dd1cefd8d0a3c96db8f25d8ede37979` |
| writer orchestration v2 | `openai_train.jsonl` | `5069d2bc9bb8f1a325faf92d26500629f7a69240f0e6bb1dd81ced075f5bf4c6` |
| writer orchestration v2 | `openai_eval.jsonl` | `e4b62da25bdbc6dda3eee90b9cf87666fcada802ce87dd0acec92e22cb58b45b` |
| writer orchestration v2 | `oracle-policy-eval.json` | `57126a63128acb5822564a1ac8eae183ae9e3558a75e26e371e3ce7064d1e247` |

## 6. Current Active Kubernetes Jobs

Only these operator jobs should remain unsuspended:

```text
k8s/kernel-operator-qwen05-lora-read-api-mcp-v1-kmp-cursor-4gpu-20260517-job.yaml
k8s/kernel-operator-qwen05-predict-read-api-mcp-v1-kmp-cursor-20260517-job.yaml
k8s/kernel-operator-qwen05-lora-writer-pre-read-v4-kmp-cursor-v2-4gpu-20260517-job.yaml
k8s/kernel-operator-qwen05-predict-writer-pre-read-v4-kmp-cursor-v2-20260517-job.yaml
k8s/kernel-operator-qwen05-lora-writer-exec-prepared-source-kind-agent-4gpu-20260517-job.yaml
k8s/kernel-operator-qwen05-predict-writer-exec-prepared-source-kind-agent-20260517-job.yaml
k8s/kernel-operator-qwen05-lora-writer-orchestration-v2-kmp-cursor-4gpu-20260517-job.yaml
k8s/kernel-operator-qwen05-predict-writer-orchestration-v2-kmp-cursor-20260517-job.yaml
```

All other `k8s/kernel-operator*.yaml` manifests are historical and suspended.
This is checked mechanically by:

```text
bash scripts/ci/check-operator-k8s-jobs.sh
```

The gate enforces the active allowlist, quarantine annotation for historical
jobs, read-only `/operator-artifacts` input datasets for current jobs, durable
`/operator-runs` output storage for adapters/predictions, and `--validate-only`
before dependency installation, output deletion, training, or adapter-backed
prediction.

## 7. Verification

Commands executed successfully:

```text
bash scripts/ci/check-operator-k8s-jobs.sh
bash scripts/operator/check_current_operator_artifacts.sh
python -m py_compile scripts/operator/predict_operator_sft.py scripts/operator/train_operator_sft_lora.py scripts/operator/prepare_operator_sft_dataset.py
cargo fmt --check
cargo test -p rehydration-mcp ingest --quiet
cargo test -p underpass-operator-shared-domain --quiet
cargo test -p underpass-operator-evaluation-cli --bin underpass_operator_contract_coverage --quiet
cargo run -p underpass-operator-evaluation-cli --bin underpass_operator_contract_coverage -- --profile read --trajectories ../rehydration-kernel-artifacts/operator/2026-05-17-kmp-cursor-contract-refresh/kernel-operator-sft-read-api-mcp-v1-kmp-cursor-20260517/eval.jsonl --fail-under 100
cargo run -p underpass-operator-evaluation-cli --bin underpass_operator_contract_coverage -- --profile writer-pre-read --trajectories ../rehydration-kernel-artifacts/operator/2026-05-17-kmp-cursor-contract-refresh/kernel-operator-sft-writer-pre-read-v4-kmp-cursor-v2-20260517/eval.jsonl --fail-under 100
cargo run -p underpass-operator-evaluation-cli --bin underpass_operator_contract_coverage -- --profile write --trajectories ../rehydration-kernel-artifacts/operator/2026-05-17-kmp-cursor-contract-refresh/kernel-operator-sft-writer-exec-v1-prepared-exec-source-kind-agent-20260517/eval.jsonl --fail-under 100
cargo run -p underpass-operator-evaluation-cli --bin underpass_operator_contract_coverage -- --profile writer-pre-read --trajectories ../rehydration-kernel-artifacts/operator/2026-05-17-kmp-cursor-contract-refresh/kernel-operator-sft-writer-orchestration-v2-kmp-cursor-20260517/eval.jsonl --fail-under 100
cargo run -p underpass-operator-evaluation-cli --bin underpass_operator_contract_coverage -- --profile write --trajectories ../rehydration-kernel-artifacts/operator/2026-05-17-kmp-cursor-contract-refresh/kernel-operator-sft-writer-orchestration-v2-kmp-cursor-20260517/eval.jsonl --fail-under 100
kubectl apply --dry-run=client -f k8s
```

The GitHub Actions workflow also runs:

```text
operator-k8s-policy -> bash scripts/ci/check-operator-k8s-jobs.sh
```

## 8. Decision

The API/MCP training data is now split into explicit valid profiles:

- read;
- writer pre-read;
- write execution;
- mixed writer orchestration.

The next training run should use only KMP-cursor/source-kind-agent artifacts.
