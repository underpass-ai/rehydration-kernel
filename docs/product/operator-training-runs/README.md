# Operator Training Runs

This directory contains one document per serious Operator training attempt.

Use:

```text
../operator-training-run-template.md
```

as the source template.

## Rules

- Create the attempt document before training starts.
- Keep failed attempts. Do not rewrite them into success stories.
- Store large artifacts outside the repository, normally under
  `../rehydration-kernel-artifacts/operator/<attempt-id>/`.
- Keep this repo focused on versioned evidence: commands, paths, hashes,
  summaries, decisions, and failure analysis.
- Do not publish or promote a model without a closed attempt document.

## Status Values

| Status | Meaning |
| --- | --- |
| `planned` | Run document exists, execution has not started. |
| `dataset-ready` | Dataset and audits are clean; training has not started. |
| `v2-golden-eval-passed` | A second golden cut passed strict prediction and policy evaluation; still requires generalization tests before promotion. |
| `v4-golden-plus-holdout-passed` | A contrastive golden cut passed both fixed golden eval and independent read-generalization holdout; still requires real KMP/MCP replay before promotion. |
| `v5-real-requested-replay-passed` | The golden read contract plus a real requested benchmark slice passed strict eval, raw de-anonymization, and live KMP/MCP replay against the deployed kernel. |
| `read-profile-passed` | The read profile passed strict offline eval and live read replay; adjacent modes such as writer pre-read may still be diagnostic. |
| `writer-pre-read-profile-passed` | The writer pre-read profile passed strict offline model-facing eval; still requires broader natural traces and live replay before promotion. |
| `writer-exec-profile-passed` | The prepared-write execution profile passed strict resolved model-facing eval and live KMP/MCP write replay; still requires mixed orchestration before promotion. |
| `writer-orchestration-profile-passed` | The mixed writer profile passed strict resolved model-facing eval across bounded pre-read and prepared write execution; live replay may still require seeded refs or a real benchmark trace. |
| `running` | Training/eval is active. |
| `paused` | Execution stopped temporarily and may resume. |
| `failed` | The run produced a negative result or hit a blocker. |
| `quarantined` | Artifacts are retained but must not be used for claims/training. |
| `baseline-only` | Useful comparison, not a release candidate. |
| `internal-only` | Useful for engineering, not external claims. |
| `promoted` | Current baseline or publication candidate. |
| `aborted` | Stopped before useful evidence. |

## Attempt Index

Add rows here as attempts are opened.

| Attempt | Status | Scope | Result |
| --- | --- | --- | --- |
| [2026-05-14-next-operator-training-draft.md](2026-05-14-next-operator-training-draft.md) | internal-only | `operator-read` mixed MemoryArena + conformance | Strong MemoryArena result, but conformance rare API/MCP rows failed; not a public candidate. |
| [2026-05-15-opread-golden-v1.md](2026-05-15-opread-golden-v1.md) | v5-real-requested-replay-passed | `operator-read` golden conformance + real requested P111 replay | V1/V3 were diagnostic; V4 passed golden and holdout; V5 mixed golden v4 with 200 requested P111 rows, reached 55/55 exact offline eval, 43/43 raw P111 exact eval, and 32/32 successful live KMP/MCP tool calls with zero missing expected refs. |
| [2026-05-16-p111-scale-gate.md](2026-05-16-p111-scale-gate.md) | read-profile-passed | MemoryArena P1.11 221-task scale gate + rare read conformance | V3 baseline was not promotable. `read-rare-v1` reached 673/673 strict predictions, 665/665 exact `read` actions, 0 invalid, 0 missing, 0 unbounded, 3/3 trace continuation, and live MCP replay over all 633 real MemoryArena eval read rows with 469/469 successful tool calls. `write_context_read` remains diagnostic at 4/8 exact. |
| [2026-05-16-read-api-mcp-v1-dataset.md](2026-05-16-read-api-mcp-v1-dataset.md) | quarantined | Synthetic API/MCP read conformance dataset | Original 2026-05-16 SFT rows used symbolic trace cursors and are quarantined under the current strict contract. The 2026-05-17 KMP-cursor replacement has 716 rows, 501 train rows, 215 eval rows, 24/24 read capability coverage, validate-only train/predict pass, and numeric KMP trace cursors. |
| [2026-05-16-writer-pre-read-gate.md](2026-05-16-writer-pre-read-gate.md) | baseline-only | Smart-writer pre-read conformance and SFT gate | Historical v1 pre-read gate. Retained as evidence only; superseded by later v4 stop/trace coverage and the mixed writer-orchestration v2 dataset. |
| [2026-05-16-writer-pre-read-p111-mixed.md](2026-05-16-writer-pre-read-p111-mixed.md) | baseline-only | MemoryArena P1.11 smart-writer pre-read rows plus `writer-pre-read-v1` conformance | Historical diagnostic mix. It measured that real P1.11 rows cover only 13/16 target capabilities and are structurally repetitive; superseded by v2/v4 gates. |
| [2026-05-16-writer-pre-read-v2-diversity-gate.md](2026-05-16-writer-pre-read-v2-diversity-gate.md) | baseline-only | Expanded writer pre-read conformance plus MemoryArena P1.11 smart-writer pre-read rows | Historical diagnostic gate with 21/21 coverage but repetitive real rows after anonymization. Superseded by v4 stop/trace coverage and the mixed writer-orchestration v2 dataset. |
| [2026-05-16-writer-pre-read-v3-dataset.md](2026-05-16-writer-pre-read-v3-dataset.md) | quarantined | Synthetic writer pre-read conformance dataset paired with `read-api-mcp-v1` | Adds a 360-row writer pre-read cut with 21/21 writer profile coverage, but training exposed that `kernel_near` rows expected exact bounds without exposing `requested_bounds`. Retained for traceability; v3b supersedes it. |
| [2026-05-16-separated-read-writer-qwen05.md](2026-05-16-separated-read-writer-qwen05.md) | internal-only | Separate Qwen 0.5B LoRA runs for read and writer-pre-read profiles | Reader reached 214/215 exact model-facing actions. Writer v3 exposed an implicit-bounds dataset bug; v3b made requested bounds visible and reached 107/108 exact actions. The evaluator now supports explicit `--model-facing-eval` so anonymized predictions are not compared against raw refs. |
| [2026-05-16-writer-pre-read-v4-stop-trace-qwen05.md](2026-05-16-writer-pre-read-v4-stop-trace-qwen05.md) | quarantined | Qwen 0.5B LoRA over cohesive writer-pre-read v4 stop-vs-trace hard negatives | Original standalone SFT rows used symbolic trace cursors and are quarantined under the current strict contract. The 2026-05-17 KMP-cursor replacement has 576 rows, 403 train rows, 173 eval rows, 21/21 writer-pre-read coverage, validate-only train/predict pass, and numeric KMP trace cursors. |
| [2026-05-16-writer-exec-v1-dataset.md](2026-05-16-writer-exec-v1-dataset.md) | baseline-only | Synthetic prepared-write execution conformance dataset | Historical full-payload writer-exec cut. It exposed copy-fidelity as the wrong target for a small Operator model; superseded by prepared-payload execution. |
| [2026-05-17-writer-exec-v1-qwen05-copy-fidelity.md](2026-05-17-writer-exec-v1-qwen05-copy-fidelity.md) | failed | Qwen 0.5B LoRA over the prepared-write execution profile | Training and prediction completed cleanly with 0 invalid JSON and 0 unbounded calls. Strict model-facing eval reached 69/70 exact actions: tool, scope, primary refs, and stop decisions were 100%, but one `kernel_write_memory` payload rewrote a `why` string by dropping one word. This is not promotable until byte-exact copy fidelity is solved or prepared-payload execution becomes deterministic outside the model. |
| [2026-05-17-writer-exec-prepared-payload-executor.md](2026-05-17-writer-exec-prepared-payload-executor.md) | dataset-ready | Deterministic prepared-payload executor for writer-exec | Changes `writer-exec` successful targets from full payload copy to compact `prepared_tool_call` decisions. The predictor and policy evaluator can resolve those decisions to final `kernel_write_memory`/`kernel_ingest` actions. Regenerated SFT has 234 rows, 108 prepared tool calls, 126 stops, 20/20 coverage, 0 no-gold findings, and 70/70 resolved oracle exact eval. |
| [2026-05-17-writer-exec-prepared-v1-qwen05.md](2026-05-17-writer-exec-prepared-v1-qwen05.md) | writer-exec-profile-passed | Qwen 0.5B LoRA over compact prepared-payload writer-exec decisions | Training completed on 4 x RTX 3090 in 106.1 s. Prediction used `--resolve-prepared-payloads` and reached 70/70 exact resolved model-facing actions, 40/40 stop, 23/23 `kernel_write_memory`, 7/7 `kernel_ingest`, 0 invalid, and 0 unbounded. Live replay exposed and fixed an invalid synthetic `source_kind`; corrected replay passed 30/30 MCP tool calls plus a real `dry_run=false` gRPC ingest/inspect smoke. The original standalone K8s jobs are now quarantined; new standalone manifests use the corrected `source-kind-agent` dataset. |
| [2026-05-17-kmp-cursor-contract-refresh.md](2026-05-17-kmp-cursor-contract-refresh.md) | dataset-ready | Strict KMP cursor/source-kind refresh for Operator training data and Kubernetes jobs | Regenerated read and writer-pre-read standalone SFT datasets with numeric KMP trace cursors, validated source-kind-agent writer-exec data, added write-profile coverage, fixed coverage over final SFT rows and `prepared_tool_call`, preserved valid artifacts outside `/tmp`, and suspended all historical `kernel-operator*.yaml` jobs outside the current allowlist. |
| [2026-05-17-writer-orchestration-v1-qwen05.md](2026-05-17-writer-orchestration-v1-qwen05.md) | quarantined | Qwen 0.5B LoRA over mixed writer pre-read plus prepared write execution | Historical run retained for comparison only. It passed the then-current offline gate, but later audit found symbolic trace cursors, weaker stop scoring, and incomplete model-facing action validation. Do not use this adapter or dataset for claims. Superseded by the KMP-cursor v2 dataset. |
| [2026-05-17-writer-orchestration-v2-kmp-cursor.md](2026-05-17-writer-orchestration-v2-kmp-cursor.md) | dataset-ready | Clean KMP-cursor mixed writer orchestration dataset | Regenerates writer orchestration with numeric KMP `Trace.next_cursor` values, namespaced about/idempotency keys, strict stop scoring, strict predictor/preparer action validation, prepared-payload visibility checks, 810 rows, 40/40 train/eval capability coverage, 0 duplicate model rows, 0 train/eval overlap, and 243/243 resolved oracle exact eval. Training has not been rerun yet. |
| [2026-05-16-writer-pre-read-v2-qwen05-diagnostic.md](2026-05-16-writer-pre-read-v2-qwen05-diagnostic.md) | failed | Qwen 0.5B diagnostic training over the `writer-pre-read-v2` mixed no-overlap cut | The initial compact split was rejected because train/eval shared 10 model-row hashes. The corrected r05 split had zero overlap and 21/21 coverage, but only 24 unique model-facing rows, weak trace/stop representation, and high `kernel_inspect` collapse risk. Training produced valid JSON but predicted `kernel_inspect` for all 85 eval rows, reaching only the majority baseline shape: 41/85 exact actions. |
