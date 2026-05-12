#!/usr/bin/env python3
"""Map operator predictions from synthetic refs back to raw kernel refs."""

from __future__ import annotations

import argparse
import json
import re
import shutil
from pathlib import Path
from typing import Any


SYNTHETIC_REF_PATTERN = re.compile(r"^ref_\d{4}$")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="De-anonymize KMP operator predictions for raw replay/eval."
    )
    parser.add_argument("--raw-trajectories", required=True, type=Path)
    parser.add_argument("--model-trajectories", required=True, type=Path)
    parser.add_argument("--predictions", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--force", action="store_true")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    if args.output.exists():
        if not args.force:
            raise SystemExit(f"output already exists: {args.output}; pass --force")
        shutil.rmtree(args.output)
    args.output.mkdir(parents=True)

    raw_by_step = read_trajectories(args.raw_trajectories)
    model_by_step = read_trajectories(args.model_trajectories)
    predictions = read_jsonl(args.predictions)

    predictions_path = args.output / "predictions.jsonl"
    audit_path = args.output / "audit.jsonl"
    failures_path = args.output / "failures.jsonl"
    written = 0
    failures = 0
    mapped_ref_total = 0

    with predictions_path.open("w", encoding="utf-8") as pred_handle, audit_path.open(
        "w", encoding="utf-8"
    ) as audit_handle, failures_path.open("w", encoding="utf-8") as failure_handle:
        for index, prediction in enumerate(predictions, start=1):
            step_id = string_field(prediction, "step_id")
            action = prediction.get("action") or prediction.get("target_action")
            if not isinstance(action, dict):
                failures += 1
                write_jsonl(
                    failure_handle,
                    {
                        "line": index,
                        "step_id": step_id,
                        "reason": "missing_action",
                    },
                )
                continue
            raw = pop_trajectory(raw_by_step, step_id)
            model = pop_trajectory(model_by_step, step_id)
            if raw is None or model is None:
                failures += 1
                write_jsonl(
                    failure_handle,
                    {
                        "line": index,
                        "step_id": step_id,
                        "reason": "missing_trajectory_pair",
                        "raw_present": raw is not None,
                        "model_present": model is not None,
                    },
                )
                continue

            try:
                ref_map = build_ref_map(model, raw)
                raw_action = replace_synthetic_refs(action, ref_map)
            except ValueError as error:
                failures += 1
                write_jsonl(
                    failure_handle,
                    {
                        "line": index,
                        "step_id": step_id,
                        "reason": "mapping_error",
                        "error": str(error),
                    },
                )
                continue

            write_jsonl(pred_handle, {"step_id": step_id, "action": raw_action})
            write_jsonl(
                audit_handle,
                {
                    "step_id": step_id,
                    "model_action": action,
                    "raw_action": raw_action,
                    "mapped_refs": sorted(ref_map),
                    "mapped_ref_count": len(ref_map),
                },
            )
            written += 1
            mapped_ref_total += len(ref_map)

    summary = {
        "deanonymizer": "kernel-operator-deanonymize-predictions-v1",
        "raw_trajectories": str(args.raw_trajectories),
        "model_trajectories": str(args.model_trajectories),
        "predictions": str(args.predictions),
        "output": str(args.output),
        "selected": len(predictions),
        "written": written,
        "failures": failures,
        "mapped_ref_total": mapped_ref_total,
    }
    (args.output / "summary.json").write_text(
        json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    print(json.dumps(summary, indent=2, sort_keys=True))
    if failures:
        raise SystemExit(f"failed to de-anonymize {failures} prediction(s)")


def read_trajectories(path: Path) -> dict[str, list[dict[str, Any]]]:
    values: dict[str, list[dict[str, Any]]] = {}
    for index, value in enumerate(read_jsonl(path), start=1):
        step_id = string_field(value, "step_id")
        values.setdefault(step_id, []).append(value)
    return values


def pop_trajectory(
    values: dict[str, list[dict[str, Any]]], step_id: str
) -> dict[str, Any] | None:
    matches = values.get(step_id)
    if not matches:
        return None
    return matches.pop(0)


def read_jsonl(path: Path) -> list[dict[str, Any]]:
    values: list[dict[str, Any]] = []
    with path.open(encoding="utf-8") as handle:
        for index, line in enumerate(handle, start=1):
            line = line.strip()
            if not line:
                continue
            value = json.loads(line)
            if not isinstance(value, dict):
                raise SystemExit(f"{path}:{index} must be a JSON object")
            values.append(value)
    return values


def string_field(value: dict[str, Any], field: str) -> str:
    result = value.get(field)
    if not isinstance(result, str) or not result:
        raise SystemExit(f"missing non-empty string field `{field}`")
    return result


def build_ref_map(model: Any, raw: Any) -> dict[str, str]:
    ref_map: dict[str, str] = {}
    collect_ref_map(model, raw, ref_map)
    return ref_map


def collect_ref_map(model: Any, raw: Any, ref_map: dict[str, str]) -> None:
    if isinstance(model, str):
        if not SYNTHETIC_REF_PATTERN.match(model):
            return
        if not isinstance(raw, str) or not raw:
            raise ValueError(f"synthetic ref {model} maps to non-string raw value")
        existing = ref_map.get(model)
        if existing is not None and existing != raw:
            raise ValueError(
                f"synthetic ref {model} maps to both {existing!r} and {raw!r}"
            )
        ref_map[model] = raw
        return
    if isinstance(model, list) and isinstance(raw, list):
        for model_item, raw_item in zip(model, raw, strict=True):
            collect_ref_map(model_item, raw_item, ref_map)
        return
    if isinstance(model, dict) and isinstance(raw, dict):
        for key, model_value in model.items():
            if key in raw:
                collect_ref_map(model_value, raw[key], ref_map)


def replace_synthetic_refs(value: Any, ref_map: dict[str, str]) -> Any:
    if isinstance(value, str):
        if SYNTHETIC_REF_PATTERN.match(value):
            mapped = ref_map.get(value)
            if mapped is None:
                raise ValueError(f"synthetic ref {value} is not visible in trajectory")
            return mapped
        return value
    if isinstance(value, list):
        return [replace_synthetic_refs(item, ref_map) for item in value]
    if isinstance(value, dict):
        return {
            key: replace_synthetic_refs(item, ref_map)
            for key, item in value.items()
        }
    return value


def write_jsonl(handle: Any, value: dict[str, Any]) -> None:
    handle.write(json.dumps(value, separators=(",", ":"), sort_keys=True) + "\n")


if __name__ == "__main__":
    main()
