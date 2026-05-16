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
