#!/usr/bin/env python3
"""Compare two Operator policy detail JSONL files.

The inputs are produced by:

  underpass_operator_policy_eval --details-output <details.jsonl>

Use this after adding or removing a training batch and re-running the same
frozen probe set. It classifies each probe as improved, regressed, stable
correct, or stable gap.
"""

from __future__ import annotations

import argparse
import json
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any


SCORE_KEYS = (
    "exact_action_correct",
    "action_type_correct",
    "tool_correct",
    "primary_refs_correct",
    "scope_correct",
    "cursor_mode_correct",
    "window_shape_correct",
    "limit_policy_correct",
    "continue_page_correct",
    "stop_correct",
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Compare two Operator policy detail JSONL files."
    )
    parser.add_argument("--baseline-details", required=True, type=Path)
    parser.add_argument("--candidate-details", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--summary-output", required=True, type=Path)
    parser.add_argument("--force", action="store_true")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    for output in (args.output, args.summary_output):
        if output.exists() and not args.force:
            raise SystemExit(f"{output} already exists; pass --force to overwrite")

    baseline = read_details(args.baseline_details)
    candidate = read_details(args.candidate_details)
    step_ids = sorted(set(baseline) | set(candidate))

    rows: list[dict[str, Any]] = []
    verdict_counts: Counter[str] = Counter()
    by_capability: dict[str, Counter[str]] = defaultdict(Counter)
    by_capability_delta: Counter[str] = Counter()

    for step_id in step_ids:
        before = baseline.get(step_id)
        after = candidate.get(step_id)
        before_score = row_score(before)
        after_score = row_score(after)
        delta = after_score - before_score
        verdict = classify(before, after, before_score, after_score)
        capability = capability_key(before, after)

        row = {
            "step_id": step_id,
            "target_capability_key": capability,
            "target_action_label": value_from_pair(
                before, after, "target_action_label"
            ),
            "baseline_status": status(before),
            "candidate_status": status(after),
            "baseline_predicted_action_label": label(before),
            "candidate_predicted_action_label": label(after),
            "baseline_score": before_score,
            "candidate_score": after_score,
            "delta": delta,
            "verdict": verdict,
            "baseline_exact": exact(before),
            "candidate_exact": exact(after),
            "baseline_invalid_reason": invalid_reason(before),
            "candidate_invalid_reason": invalid_reason(after),
        }
        rows.append(row)
        verdict_counts[verdict] += 1
        by_capability[capability][verdict] += 1
        by_capability_delta[capability] += delta

    summary = {
        "reporter": "kernel-operator-policy-delta-v1",
        "baseline_details": str(args.baseline_details),
        "candidate_details": str(args.candidate_details),
        "total": len(step_ids),
        "verdicts": dict(sorted(verdict_counts.items())),
        "net_score_delta": sum(row["delta"] for row in rows),
        "by_capability": {
            capability: {
                "verdicts": dict(sorted(counts.items())),
                "net_score_delta": by_capability_delta[capability],
            }
            for capability, counts in sorted(by_capability.items())
        },
        "interpretation": {
            "improved": "candidate scored higher than baseline on the same probe",
            "regressed": "candidate scored lower than baseline on the same probe",
            "stable_correct": "candidate stayed exact on a probe that is already solved",
            "stable_gap": "candidate stayed non-exact; this capability still needs data or prompt work",
            "missing_baseline": "baseline detail row was absent",
            "missing_candidate": "candidate detail row was absent",
        },
    }

    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("w", encoding="utf-8") as handle:
        for row in rows:
            handle.write(json.dumps(row, sort_keys=True, separators=(",", ":")))
            handle.write("\n")

    args.summary_output.parent.mkdir(parents=True, exist_ok=True)
    args.summary_output.write_text(
        json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )

    print(json.dumps(summary, indent=2, sort_keys=True))


def read_details(path: Path) -> dict[str, dict[str, Any]]:
    rows: dict[str, dict[str, Any]] = {}
    with path.open("r", encoding="utf-8") as handle:
        for line_number, line in enumerate(handle, start=1):
            if not line.strip():
                continue
            row = json.loads(line)
            step_id = row.get("step_id")
            if not isinstance(step_id, str) or not step_id:
                raise SystemExit(f"{path}:{line_number} missing string step_id")
            if step_id in rows:
                raise SystemExit(f"{path}:{line_number} duplicate step_id {step_id}")
            rows[step_id] = row
    return rows


def row_score(row: dict[str, Any] | None) -> int:
    if row is None:
        return 0
    score = 0
    if row.get("prediction_status") == "valid":
        score += 1
    score_fields = row.get("score")
    if not isinstance(score_fields, dict):
        return score
    for key in SCORE_KEYS:
        if score_fields.get(key) is True:
            score += 1
    return score


def classify(
    before: dict[str, Any] | None,
    after: dict[str, Any] | None,
    before_score: int,
    after_score: int,
) -> str:
    if before is None:
        return "missing_baseline"
    if after is None:
        return "missing_candidate"
    if after_score > before_score:
        return "improved"
    if after_score < before_score:
        return "regressed"
    if exact(after):
        return "stable_correct"
    return "stable_gap"


def capability_key(
    before: dict[str, Any] | None, after: dict[str, Any] | None
) -> str:
    return value_from_pair(before, after, "target_capability_key") or "unknown"


def value_from_pair(
    before: dict[str, Any] | None, after: dict[str, Any] | None, key: str
) -> str | None:
    for row in (after, before):
        value = row.get(key) if row else None
        if isinstance(value, str):
            return value
    return None


def status(row: dict[str, Any] | None) -> str | None:
    value = row.get("prediction_status") if row else None
    return value if isinstance(value, str) else None


def label(row: dict[str, Any] | None) -> str | None:
    value = row.get("predicted_action_label") if row else None
    return value if isinstance(value, str) else None


def invalid_reason(row: dict[str, Any] | None) -> str | None:
    value = row.get("invalid_reason") if row else None
    return value if isinstance(value, str) else None


def exact(row: dict[str, Any] | None) -> bool:
    score = row.get("score") if row else None
    return isinstance(score, dict) and score.get("exact_action_correct") is True


if __name__ == "__main__":
    main()
