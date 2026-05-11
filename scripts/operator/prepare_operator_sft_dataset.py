#!/usr/bin/env python3
"""Prepare SFT JSONL data for the KMP tool-operator model.

The input is `kernel-operator-trajectory-v1` JSONL exported by
`kernel_operator_trajectory_export`. The output is a conversational SFT dataset:

  {"messages": [{"role": "system", ...}, {"role": "user", ...}, {"role": "assistant", ...}]}

The user prompt never includes `target_action`, observed tool output, benchmark
gold answers, or hidden raw memory. The assistant message is exactly the
expected action wrapper consumed by the policy evaluator.
"""

from __future__ import annotations

import argparse
import json
import random
import shutil
from pathlib import Path
from typing import Any


SYSTEM_PROMPT = """You operate Underpass Kernel Memory Protocol tools.

Return exactly one JSON object with an `action` field.
Do not explain. Do not include markdown. Do not invent refs, scopes, or hidden memory.

Allowed action shapes:

{"action":{"type":"tool_call","tool":"kernel_near","arguments":{"about":"...","around":{"ref":"..."},"dimensions":{"mode":"all","scope":"current_about"},"include":{"evidence":true,"raw_refs":false,"relations":true},"limit":{"entries":12,"tokens":2400},"budget":{"depth":3,"tokens":2400},"window":{"before_entries":6,"after_entries":0}}}}

{"action":{"type":"tool_call","tool":"kernel_inspect","arguments":{"ref":"...","include":{"details":true,"incoming":true,"outgoing":true,"raw":false}}}}

{"action":{"type":"tool_call","tool":"kernel_trace","arguments":{"from":"...","to":"...","goal":"Kernel operator trace probe","budget":{"depth":1,"tokens":1600}}}}

{"action":{"type":"stop","answer_policy":"evidence_or_unknown","final_refs":["..."],"reason":"sufficient_evidence"}}

Rules:
- Use only tools present in `allowed_tools`.
- Use only refs visible in `current_ref`, `trace_target_ref`, `candidate_refs`, `candidate_ref_details`, `known_refs`, or `last_observed_refs`.
- Prefer `candidate_ref_details` when choosing between writer candidates.
- Every tool call must be bounded.
- For `kernel_near`, `arguments.about` must equal the top-level `about` value exactly.
- Do not use `current_ref` as `arguments.about`.
- `kernel_inspect.include.raw` must be false.
"""


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Prepare SFT data for the KMP tool-operator model."
    )
    parser.add_argument("--trajectories", required=True, type=Path, action="append")
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--limit", type=int, default=None)
    parser.add_argument("--offset", type=int, default=0)
    parser.add_argument("--eval-ratio", type=float, default=0.1)
    parser.add_argument("--split-mode", choices=["row", "group"], default="row")
    parser.add_argument(
        "--group-key",
        choices=["task_id", "task_type", "task_family", "mode", "about", "run_id"],
        default="task_id",
        help="Grouping key used when --split-mode=group.",
    )
    parser.add_argument(
        "--eval-group-values",
        default=None,
        help=(
            "Comma-separated group values to reserve for eval when "
            "--split-mode=group. Fails fast if any value is absent."
        ),
    )
    parser.add_argument(
        "--eval-group-values-file",
        type=Path,
        default=None,
        help=(
            "File with one group value per line to reserve for eval when "
            "--split-mode=group. Blank lines and # comments are ignored."
        ),
    )
    parser.add_argument("--seed", type=int, default=42)
    parser.add_argument("--max-refs", type=int, default=32)
    parser.add_argument(
        "--anonymize-refs",
        action="store_true",
        help="Replace model-facing refs with stable synthetic ids per trajectory.",
    )
    parser.add_argument(
        "--require-visible-target-refs",
        action="store_true",
        help=(
            "Drop rows whose target action refers to refs that are not visible "
            "in current_ref, trace_target_ref, candidate_refs, "
            "candidate_ref_details, known_refs, or last_observed_refs."
        ),
    )
    parser.add_argument("--force", action="store_true")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    if args.eval_ratio < 0 or args.eval_ratio >= 1:
        raise SystemExit("--eval-ratio must be >= 0 and < 1")
    if args.output.exists():
        if not args.force:
            raise SystemExit(f"output already exists: {args.output}; pass --force")
        shutil.rmtree(args.output)
    args.output.mkdir(parents=True)

    source_trajectories = []
    for path in args.trajectories:
        source_trajectories.extend(read_jsonl(path))
    trajectories = source_trajectories
    selected = trajectories[args.offset :]
    if args.limit is not None:
        selected = selected[: args.limit]

    dropped_rows: list[dict[str, Any]] = []
    if args.require_visible_target_refs:
        visible_selected = []
        for item in selected:
            missing_refs = missing_visible_target_refs(item)
            if missing_refs:
                dropped_rows.append(
                    {
                        "step_id": item.get("step_id"),
                        "run_id": item.get("run_id"),
                        "mode": item.get("mode"),
                        "tool": item.get("target_action", {}).get("tool"),
                        "action_type": item.get("target_action", {}).get("type"),
                        "missing_refs": sorted(missing_refs),
                    }
                )
            else:
                visible_selected.append(item)
        selected = visible_selected

    pairs = [build_pair(item, args.max_refs, args.anonymize_refs) for item in selected]
    train_pairs, eval_pairs, split_summary = split_pairs(pairs, args)
    eval_rows = [pair[0] for pair in eval_pairs]
    train_rows = [pair[0] for pair in train_pairs]
    ordered_pairs = train_pairs + eval_pairs
    rows = [pair[0] for pair in ordered_pairs]
    eval_model_trajectories = [pair[2] for pair in eval_pairs]
    train_model_trajectories = [pair[2] for pair in train_pairs]
    model_trajectories = [pair[2] for pair in ordered_pairs]
    eval_trajectories = [pair[1] for pair in eval_pairs]
    train_trajectories = [pair[1] for pair in train_pairs]
    split_trajectories = [pair[1] for pair in ordered_pairs]

    write_jsonl(args.output / "train.jsonl", train_rows)
    write_jsonl(args.output / "eval.jsonl", eval_rows)
    write_jsonl(args.output / "all.jsonl", rows)
    write_jsonl(args.output / "train_trajectories.jsonl", train_trajectories)
    write_jsonl(args.output / "eval_trajectories.jsonl", eval_trajectories)
    write_jsonl(args.output / "all_trajectories.jsonl", split_trajectories)
    write_jsonl(args.output / "train_model_trajectories.jsonl", train_model_trajectories)
    write_jsonl(args.output / "eval_model_trajectories.jsonl", eval_model_trajectories)
    write_jsonl(args.output / "all_model_trajectories.jsonl", model_trajectories)
    write_jsonl(args.output / "dropped_non_visible_target_refs.jsonl", dropped_rows)
    write_openai_jsonl(args.output / "openai_train.jsonl", train_rows)
    write_openai_jsonl(args.output / "openai_eval.jsonl", eval_rows)
    write_openai_jsonl(args.output / "openai_all.jsonl", rows)
    summary = {
        "dataset": "kernel-operator-sft-v1",
        "source": [str(path) for path in args.trajectories],
        "output": str(args.output),
        "total_source": len(source_trajectories),
        "selected": len(selected),
        "train": len(train_rows),
        "eval": len(eval_rows),
        "openai_train": str(args.output / "openai_train.jsonl"),
        "openai_eval": str(args.output / "openai_eval.jsonl"),
        "seed": args.seed,
        "max_refs": args.max_refs,
        "anonymize_refs": args.anonymize_refs,
        "require_visible_target_refs": args.require_visible_target_refs,
        "dropped_non_visible_target_refs": len(dropped_rows),
        "model_trajectories": {
            "train": str(args.output / "train_model_trajectories.jsonl"),
            "eval": str(args.output / "eval_model_trajectories.jsonl"),
            "all": str(args.output / "all_model_trajectories.jsonl"),
        },
        "audit_trajectories": {
            "train": str(args.output / "train_trajectories.jsonl"),
            "eval": str(args.output / "eval_trajectories.jsonl"),
            "all": str(args.output / "all_trajectories.jsonl"),
        },
        **split_summary,
        "by_mode": count_by(rows, "mode"),
        "by_task_family": count_by(rows, "task_family"),
        "by_action": count_actions(rows),
    }
    (args.output / "summary.json").write_text(
        json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    print(json.dumps(summary, indent=2, sort_keys=True))


def read_jsonl(path: Path) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    with path.open(encoding="utf-8") as handle:
        for line_no, line in enumerate(handle, start=1):
            line = line.strip()
            if not line:
                continue
            value = json.loads(line)
            if "target_action" not in value:
                raise ValueError(f"{path}:{line_no} missing target_action")
            rows.append(value)
    return rows


def build_pair(
    item: dict[str, Any],
    max_refs: int,
    anonymize_refs: bool,
) -> tuple[dict[str, Any], dict[str, Any], dict[str, Any]]:
    if not anonymize_refs:
        return to_sft_row(item, max_refs), item, item
    anonymized = anonymize_trajectory(item)
    return to_sft_row(anonymized, max_refs), item, anonymized


def split_pairs(
    pairs: list[tuple[dict[str, Any], dict[str, Any], dict[str, Any]]],
    args: argparse.Namespace,
) -> tuple[
    list[tuple[dict[str, Any], dict[str, Any], dict[str, Any]]],
    list[tuple[dict[str, Any], dict[str, Any], dict[str, Any]]],
    dict[str, Any],
]:
    rng = random.Random(args.seed)
    eval_group_values = requested_eval_group_values(args)
    if eval_group_values and args.split_mode != "group":
        raise ValueError("--eval-group-values requires --split-mode=group")
    if args.split_mode == "row":
        shuffled = list(pairs)
        rng.shuffle(shuffled)
        eval_size = int(round(len(shuffled) * args.eval_ratio))
        if len(shuffled) > 1 and args.eval_ratio > 0:
            eval_size = max(1, eval_size)
        return shuffled[eval_size:], shuffled[:eval_size], {
            "split_mode": "row",
            "group_key": None,
            "train_groups": None,
            "eval_groups": None,
        }

    groups: dict[str, list[tuple[dict[str, Any], dict[str, Any], dict[str, Any]]]] = {}
    for pair in pairs:
        group = group_value(pair[1], args.group_key)
        groups.setdefault(group, []).append(pair)
    group_ids = sorted(groups)
    if eval_group_values:
        group_set = set(group_ids)
        missing_groups = sorted(set(eval_group_values) - group_set)
        if missing_groups:
            raise ValueError(
                "explicit eval group values are not present: "
                + ", ".join(missing_groups)
            )
        eval_group_value_set = set(eval_group_values)
        eval_group_ids = [
            group for group in group_ids if group in eval_group_value_set
        ]
        train_group_ids = [
            group for group in group_ids if group not in eval_group_value_set
        ]
        if not eval_group_ids:
            raise ValueError("explicit eval group selection produced an empty eval split")
        if not train_group_ids:
            raise ValueError("explicit eval group selection produced an empty train split")
        return (
            [pair for group_id in train_group_ids for pair in groups[group_id]],
            [pair for group_id in eval_group_ids for pair in groups[group_id]],
            {
                "split_mode": "group",
                "group_key": args.group_key,
                "train_groups": len(train_group_ids),
                "eval_groups": len(eval_group_ids),
                "eval_group_selection": "explicit",
                "eval_group_values": eval_group_ids,
            },
        )

    rng.shuffle(group_ids)
    target_eval_rows = int(round(len(pairs) * args.eval_ratio))
    if len(pairs) > 1 and args.eval_ratio > 0:
        target_eval_rows = max(1, target_eval_rows)

    eval_group_ids: list[str] = []
    eval_pairs: list[tuple[dict[str, Any], dict[str, Any], dict[str, Any]]] = []
    train_pairs: list[tuple[dict[str, Any], dict[str, Any], dict[str, Any]]] = []
    for group_id in group_ids:
        if len(eval_pairs) < target_eval_rows:
            eval_group_ids.append(group_id)
            eval_pairs.extend(groups[group_id])
        else:
            train_pairs.extend(groups[group_id])

    if not train_pairs and eval_pairs:
        moved_group = eval_group_ids.pop()
        train_pairs.extend(groups[moved_group])
        eval_pairs = [pair for group_id in eval_group_ids for pair in groups[group_id]]

    return train_pairs, eval_pairs, {
        "split_mode": "group",
        "group_key": args.group_key,
        "train_groups": len(group_ids) - len(eval_group_ids),
        "eval_groups": len(eval_group_ids),
        "eval_group_selection": "ratio",
        "eval_group_values": eval_group_ids,
    }


def requested_eval_group_values(args: argparse.Namespace) -> list[str]:
    values: list[str] = []
    if args.eval_group_values:
        values.extend(
            value.strip()
            for value in args.eval_group_values.split(",")
            if value.strip()
        )
    if args.eval_group_values_file:
        with args.eval_group_values_file.open(encoding="utf-8") as handle:
            for line in handle:
                value = line.strip()
                if value and not value.startswith("#"):
                    values.append(value)
    return sorted(set(values))


def group_value(item: dict[str, Any], key: str) -> str:
    if key == "run_id":
        return str(item.get("run_id", "missing"))
    if key == "about":
        return str(item.get("about", "missing"))
    if key == "task_family":
        return str(item.get("task_family", "missing"))
    if key == "mode":
        return str(item.get("mode", "missing"))
    if key == "task_type":
        task_type = (
            item.get("visible_state", {})
            .get("task", {})
            .get("task_type")
        )
        if task_type is not None:
            return str(task_type)
        about = str(item.get("about", "missing"))
        marker = ":task_type:"
        if marker in about:
            return about.rsplit(marker, 1)[-1].split(":", 1)[0]
        return about
    task_id = (
        item.get("visible_state", {})
        .get("task", {})
        .get("task_id")
    )
    if task_id is not None:
        return str(task_id)
    about = str(item.get("about", "missing"))
    marker = ":task:"
    if marker in about:
        return about.rsplit(marker, 1)[-1].split(":", 1)[0]
    return about


def to_sft_row(item: dict[str, Any], max_refs: int) -> dict[str, Any]:
    visible_state = compact_visible_state(item["visible_state"], max_refs)
    user_payload = {
        "task_family": item["task_family"],
        "mode": item["mode"],
        "about": item["about"],
        "allowed_tools": item["allowed_tools"],
        "visible_state": visible_state,
    }
    assistant_payload = {"action": item["target_action"]}
    return {
        "id": item["step_id"],
        "run_id": item["run_id"],
        "step_id": item["step_id"],
        "task_family": item["task_family"],
        "mode": item["mode"],
        "messages": [
            {"role": "system", "content": SYSTEM_PROMPT},
            {
                "role": "user",
                "content": json.dumps(user_payload, separators=(",", ":"), sort_keys=True),
            },
            {
                "role": "assistant",
                "content": json.dumps(
                    assistant_payload, separators=(",", ":"), sort_keys=True
                ),
            },
        ],
    }


def anonymize_trajectory(item: dict[str, Any]) -> dict[str, Any]:
    refs: dict[str, str] = {}

    def ref_id(value: str) -> str:
        if value not in refs:
            refs[value] = f"ref_{len(refs) + 1:04d}"
        return refs[value]

    def rewrite(value: Any) -> Any:
        if isinstance(value, str):
            if looks_like_ref(value):
                return ref_id(value)
            return value
        if isinstance(value, list):
            return [rewrite(item) for item in value]
        if isinstance(value, dict):
            return {key: rewrite(child) for key, child in value.items()}
        return value

    anonymized = json.loads(json.dumps(item))
    if isinstance(anonymized.get("about"), str) and looks_like_ref(anonymized["about"]):
        anonymized["about"] = ref_id(anonymized["about"])
    for key in ("visible_state", "target_action", "observed_outcome", "quality"):
        if key in anonymized:
            anonymized[key] = rewrite(anonymized[key])
    anonymized["ref_anonymization"] = {
        "mode": "per_trajectory",
        "ref_count": len(refs),
    }
    return anonymized


def looks_like_ref(value: str) -> bool:
    return (
        value.startswith("memoryarena:run:")
        or value.startswith("longmemeval:")
        or value.startswith("memoryagentbench:")
        or ":subtask:" in value
        or ":task:" in value
    )


def missing_visible_target_refs(item: dict[str, Any]) -> set[str]:
    visible = visible_refs(item)
    return {ref for ref in target_primary_refs(item) if ref not in visible}


def visible_refs(item: dict[str, Any]) -> set[str]:
    state = item.get("visible_state", {})
    refs: set[str] = set()
    for key in ("current_ref", "trace_target_ref"):
        value = state.get(key)
        if isinstance(value, str):
            refs.add(value)
    for key in ("candidate_refs", "known_refs", "last_observed_refs"):
        values = state.get(key)
        if isinstance(values, list):
            refs.update(value for value in values if isinstance(value, str))
    candidate_details = state.get("candidate_ref_details")
    if isinstance(candidate_details, list):
        for detail in candidate_details:
            if isinstance(detail, dict) and isinstance(detail.get("ref"), str):
                refs.add(detail["ref"])
    return refs


def target_primary_refs(item: dict[str, Any]) -> set[str]:
    action = item.get("target_action", {})
    if action.get("type") == "stop":
        refs = action.get("final_refs", [])
        return {ref for ref in refs if isinstance(ref, str)}
    if action.get("type") != "tool_call":
        return set()

    args = action.get("arguments", {})
    refs: set[str] = set()
    for key in ("ref", "from", "to"):
        value = args.get(key)
        if isinstance(value, str):
            refs.add(value)
    around = args.get("around")
    if isinstance(around, dict) and isinstance(around.get("ref"), str):
        refs.add(around["ref"])
    return refs


def compact_visible_state(value: dict[str, Any], max_refs: int) -> dict[str, Any]:
    compact = json.loads(json.dumps(value))
    for key in ("candidate_refs", "candidate_ref_details", "known_refs", "last_observed_refs"):
        refs = compact.get(key)
        if isinstance(refs, list) and len(refs) > max_refs:
            compact[f"{key}_truncated"] = len(refs) - max_refs
            compact[key] = refs[:max_refs]
    return compact


def write_jsonl(path: Path, rows: list[dict[str, Any]]) -> None:
    with path.open("w", encoding="utf-8") as handle:
        for row in rows:
            handle.write(json.dumps(row, separators=(",", ":"), sort_keys=True))
            handle.write("\n")


def write_openai_jsonl(path: Path, rows: list[dict[str, Any]]) -> None:
    with path.open("w", encoding="utf-8") as handle:
        for row in rows:
            handle.write(
                json.dumps(
                    {"messages": row["messages"]},
                    separators=(",", ":"),
                    sort_keys=True,
                )
            )
            handle.write("\n")


def count_by(rows: list[dict[str, Any]], key: str) -> dict[str, int]:
    counts: dict[str, int] = {}
    for row in rows:
        value = str(row.get(key, "unknown"))
        counts[value] = counts.get(value, 0) + 1
    return dict(sorted(counts.items()))


def count_actions(rows: list[dict[str, Any]]) -> dict[str, int]:
    counts: dict[str, int] = {}
    for row in rows:
        assistant = row["messages"][-1]["content"]
        action = json.loads(assistant)["action"]
        if action.get("type") == "tool_call":
            label = f"tool_call:{action.get('tool', 'unknown')}"
        else:
            label = str(action.get("type", "unknown"))
        counts[label] = counts.get(label, 0) + 1
    return dict(sorted(counts.items()))


if __name__ == "__main__":
    main()
