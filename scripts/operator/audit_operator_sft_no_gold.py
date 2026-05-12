#!/usr/bin/env python3
"""Fail fast when model-facing Operator SFT rows contain gold/internal fields."""

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path
from typing import Any


DEFAULT_FORBIDDEN = [
    "target_action",
    "observed_outcome",
    "expected_answer",
    "expected_answer_turn_refs",
    "answer_session_ids",
    "answer_session_refs",
    "has_answer",
    "gold_answer",
    "gold_turn_refs",
    r"\bgold\b",
]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Audit model-facing Operator SFT JSONL for gold/internal leakage."
    )
    parser.add_argument("jsonl", type=Path, nargs="+")
    parser.add_argument(
        "--forbidden",
        action="append",
        default=[],
        help="Additional forbidden regex. Can be repeated.",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=None,
        help="Optional JSON report path.",
    )
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    patterns = [re.compile(pattern, re.IGNORECASE) for pattern in DEFAULT_FORBIDDEN]
    patterns.extend(re.compile(pattern, re.IGNORECASE) for pattern in args.forbidden)

    findings: list[dict[str, Any]] = []
    rows = 0
    for path in args.jsonl:
        for line_no, row in read_jsonl(path):
            rows += 1
            content = model_facing_content(row)
            for pattern in patterns:
                if pattern.search(content):
                    findings.append(
                        {
                            "path": str(path),
                            "line": line_no,
                            "pattern": pattern.pattern,
                            "id": row.get("id") or row.get("step_id"),
                        }
                    )

    report = {
        "auditor": "operator-sft-no-gold-audit-v1",
        "rows": rows,
        "files": [str(path) for path in args.jsonl],
        "forbidden_patterns": [pattern.pattern for pattern in patterns],
        "findings": findings,
        "finding_count": len(findings),
    }
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(json.dumps(report, indent=2, sort_keys=True))
    if findings:
        raise SystemExit(1)


def read_jsonl(path: Path) -> list[tuple[int, dict[str, Any]]]:
    rows: list[tuple[int, dict[str, Any]]] = []
    with path.open(encoding="utf-8") as handle:
        for line_no, line in enumerate(handle, start=1):
            line = line.strip()
            if not line:
                continue
            rows.append((line_no, json.loads(line)))
    return rows


def model_facing_content(row: dict[str, Any]) -> str:
    messages = row.get("messages")
    if isinstance(messages, list):
        return "\n".join(
            message.get("content", "")
            for message in messages
            if isinstance(message, dict)
        )
    return json.dumps(row, sort_keys=True)


if __name__ == "__main__":
    main()
