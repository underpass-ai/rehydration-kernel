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
import shutil
from pathlib import Path
from typing import Any


SYSTEM_PROMPT = """You operate Underpass Kernel Memory Protocol tools.

Return exactly one JSON object with an `action` field.
Do not explain. Do not include markdown. Do not invent refs, scopes, or hidden memory.

Allowed action shapes:

{"action":{"type":"tool_call","tool":"kernel_wake","arguments":{"about":"...","role":"operator","intent":"...","dimensions":{"mode":"all","scope":"current_about"},"budget":{"depth":2,"tokens":2400}}}}

{"action":{"type":"tool_call","tool":"kernel_ask","arguments":{"about":"...","answer_policy":"evidence_or_unknown","dimensions":{"mode":"all","scope":"current_about"},"question":"...","budget":{"tokens":2400}}}}

{"action":{"type":"tool_call","tool":"kernel_ask","arguments":{"about":"...","answer_policy":"show_conflicts","dimensions":{"mode":"only","scope":"current_about","include":["..."]},"question":"...","budget":{"tokens":2400}}}}

{"action":{"type":"tool_call","tool":"kernel_near","arguments":{"about":"...","around":{"ref":"..."},"dimensions":{"mode":"all","scope":"current_about"},"include":{"evidence":true,"raw_refs":false,"relations":true},"limit":{"entries":12,"tokens":2400},"budget":{"depth":3,"tokens":2400},"window":{"before_entries":6,"after_entries":0}}}}

{"action":{"type":"tool_call","tool":"kernel_goto","arguments":{"about":"...","at":{"ref":"..."},"dimensions":{"mode":"all","scope":"current_about"},"include":{"evidence":true,"raw_refs":false,"relations":true},"limit":{"entries":12,"tokens":2400},"budget":{"depth":3,"tokens":2400},"window":{"before_entries":6,"after_entries":0}}}}

{"action":{"type":"tool_call","tool":"kernel_rewind","arguments":{"about":"...","from":{"ref":"..."},"dimensions":{"mode":"all","scope":"current_about"},"include":{"evidence":true,"raw_refs":false,"relations":true},"limit":{"entries":12,"tokens":2400},"budget":{"depth":3,"tokens":2400},"window":{"before_entries":6,"after_entries":0}}}}

{"action":{"type":"tool_call","tool":"kernel_forward","arguments":{"about":"...","from":{"ref":"..."},"dimensions":{"mode":"all","scope":"current_about"},"include":{"evidence":true,"raw_refs":false,"relations":true},"limit":{"entries":12,"tokens":2400},"budget":{"depth":3,"tokens":2400},"window":{"before_entries":6,"after_entries":0}}}}

{"action":{"type":"tool_call","tool":"kernel_trace","arguments":{"from":"...","to":"...","goal":"Kernel operator trace probe","role":"operator","budget":{"depth":1,"tokens":1600},"page":{"entries":16}}}}

{"action":{"type":"tool_call","tool":"kernel_inspect","arguments":{"ref":"...","include":{"details":true,"incoming":true,"outgoing":true,"raw":false}}}}

{"action":{"type":"tool_call","tool":"kernel_write_memory","arguments":{"about":"...","intent":"record_decision","actor":"...","observed_at":"...","scope":{"process":"..."},"current":{"kind":"decision","summary":"...","evidence":"..."},"connect_to":[{"ref":"...","rel":"chosen_because","class":"causal","why":"...","evidence":"...","confidence":"high"}],"read_context":{"inspected_refs":["..."]},"idempotency_key":"...","options":{"dry_run":true,"strict":true}}}}

{"action":{"type":"tool_call","tool":"kernel_ingest","arguments":{"about":"...","memory":{"dimensions":[{"id":"...","kind":"task"}],"entries":[{"id":"...","kind":"decision","text":"..."}],"relations":[{"from":"...","to":"...","rel":"chosen_because","class":"causal","why":"...","evidence":"..."}],"evidence":[{"id":"...","supports":["..."],"text":"..."}]},"provenance":{"source_kind":"agent","source_agent":"...","observed_at":"...","correlation_id":"...","causation_id":"..."},"idempotency_key":"...","dry_run":true}}}

{"action":{"type":"stop","answer_policy":"evidence_or_unknown","final_refs":["..."],"reason":"sufficient_evidence"}}

Rules:
- Use only tools present in `allowed_tools`.
- Use only refs visible in `current_ref`, `trace_target_ref`, `candidate_refs`, `candidate_ref_details`, `known_refs`, `last_observed_refs`, or `read_context`.
- If `visible_state` contains `requested_wake`, `requested_ask`, `requested_move`, `requested_scope`, `requested_bounds`, `requested_trace`, `inspection_request`, or `requested_stop`, copy those requested fields exactly into the matching action.
- If `requested_wake` is present, call `kernel_wake`; do not convert it into `kernel_near` even when `current_ref` is visible or the previous tool was `kernel_near`.
- If `requested_move` is present, its `kind` is the tool to call and its `cursor_key` is the cursor argument name.
- If `requested_trace`, `inspection_request`, or `requested_stop` is present, choose `kernel_trace`, `kernel_inspect`, or `stop` respectively.
- Supported ask `answer_policy` values are `evidence_or_unknown` and `show_conflicts`; do not invent aliases.
- For `dimensions.scope=abouts`, `abouts` must be a flat list of about ids.
- Dimension filters such as `include` and `exclude` belong only inside `arguments.dimensions`; never create top-level dimension filter fields.
- Tool result include flags belong only in `arguments.include`; do not nest `arguments.include`, `limit`, or `window` inside dimension filters.
- Prefer `candidate_ref_details` when choosing between writer candidates.
- Every tool call must be bounded.
- For tools with `arguments.about`, that value must equal the top-level `about` value exactly.
- Do not use `current_ref` as `arguments.about`.
- `kernel_inspect` arguments must use the key `ref`, never `an`, `id`, or `target`.
- `kernel_inspect.include.raw` must be false.
- Rich `kernel_write_memory.connect_to` targets require visible evidence and read_context proof.
- If a rich write lacks read_context proof, stop instead of inventing a relation.
- Use an anemic relation such as `follows` only when no richer relation is justified.
- Use `kernel_ingest` only when a complete typed memory payload is already visible.
"""

FNV64_OFFSET = 0xCBF29CE484222325
FNV64_PRIME = 0x100000001B3
FNV64_MASK = 0xFFFFFFFFFFFFFFFF

FORBIDDEN_MODEL_VISIBLE_STRING_VALUES = {
    "recorded_pre_read_argument",
    "writer_candidate_pool",
    "writer_candidate_quality_target",
    "writer_candidate_relation_target",
    "writer_read_context_inspected",
    "writer_read_context_temporal",
    "writer_target_ref",
}

FORBIDDEN_MODEL_VISIBLE_STRING_PREFIXES = (
    "writer_candidate_",
    "writer_read_context_",
)

READ_REQUIRED_CAPABILITIES = (
    "tool:kernel_wake",
    "tool:kernel_ask",
    "tool:kernel_near",
    "tool:kernel_goto",
    "tool:kernel_rewind",
    "tool:kernel_forward",
    "tool:kernel_trace",
    "tool:kernel_inspect",
    "tool:stop",
    "cursor:ref",
    "cursor:time",
    "cursor:sequence",
    "dimensions.mode:all",
    "dimensions.mode:only",
    "dimensions.mode:except",
    "dimensions.scope:current_about",
    "dimensions.scope:abouts",
    "dimensions.scope:all_abouts",
    "trace.page:first",
    "trace.page:continue",
    "window:expand",
    "window:shrink",
    "window:stop_sufficient",
    "inspect.raw:false",
)

FULL_REQUIRED_CAPABILITIES = READ_REQUIRED_CAPABILITIES + (
    "tool:kernel_ingest",
    "tool:kernel_write_memory",
    "write:relation_quality",
    "write:read_context_proof",
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Prepare SFT data for the KMP tool-operator model."
    )
    parser.add_argument("--trajectories", required=True, type=Path, action="append")
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--limit", type=int, default=None)
    parser.add_argument("--offset", type=int, default=0)
    parser.add_argument(
        "--include-mode",
        action="append",
        default=[],
        help=(
            "Keep only trajectories whose top-level mode matches this value. "
            "May be passed more than once."
        ),
    )
    parser.add_argument(
        "--exclude-mode",
        action="append",
        default=[],
        help=(
            "Drop trajectories whose top-level mode matches this value. "
            "May be passed more than once."
        ),
    )
    parser.add_argument("--eval-ratio", type=float, default=0.1)
    parser.add_argument("--split-mode", choices=["row", "group"], default="row")
    parser.add_argument(
        "--group-key",
        choices=[
            "task_id",
            "task_or_step",
            "task_type",
            "task_family",
            "mode",
            "about",
            "run_id",
        ],
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
    parser.add_argument(
        "--capability-split-profile",
        choices=["read", "full"],
        default=None,
        help=(
            "When set with --split-mode=group, seed the eval split with groups "
            "that cover the required KMP/MCP operator capabilities for this "
            "profile before filling by --eval-ratio."
        ),
    )
    parser.add_argument(
        "--require-eval-capability-coverage",
        action="store_true",
        help=(
            "Fail fast unless the eval split covers every required capability "
            "from --capability-split-profile."
        ),
    )
    parser.add_argument(
        "--require-train-capability-coverage",
        action="store_true",
        help=(
            "Fail fast unless the train split covers every required capability "
            "from --capability-split-profile."
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
    parser.add_argument(
        "--require-visible-target-cursors",
        action="store_true",
        help=(
            "Drop rows whose target action uses time, sequence, or trace page "
            "cursors that are not visible in visible_state."
        ),
    )
    parser.add_argument(
        "--inject-target-request-fields",
        action="store_true",
        help=(
            "Project target_action into model-visible requested_* fields. Use only "
            "for operator contract translation/replay smokes, not benchmark "
            "decision claims."
        ),
    )
    parser.add_argument("--force", action="store_true")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    if args.eval_ratio < 0 or args.eval_ratio >= 1:
        raise SystemExit("--eval-ratio must be >= 0 and < 1")
    if (
        args.require_eval_capability_coverage
        or args.require_train_capability_coverage
    ) and not args.capability_split_profile:
        raise SystemExit(
            "capability coverage requirements need --capability-split-profile"
        )
    if args.capability_split_profile and args.split_mode != "group":
        raise SystemExit("--capability-split-profile requires --split-mode=group")
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
    selected = filter_by_mode(selected, args.include_mode, args.exclude_mode)
    if not selected:
        raise SystemExit("mode filters selected zero trajectories")
    if args.inject_target_request_fields:
        selected = [inject_target_request_fields(item) for item in selected]
    validate_unique_step_ids(selected, "selected trajectories")
    debug_audit = {
        item["step_id"]: build_debug_audit_row(item)
        for item in selected
    }

    dropped_rows: list[dict[str, Any]] = []
    if args.require_visible_target_refs:
        visible_selected = []
        for item in selected:
            missing_refs = missing_visible_target_refs(item)
            if missing_refs:
                mark_debug_drop(
                    debug_audit,
                    item,
                    "non_visible_target_refs",
                    {"missing_refs": sorted(missing_refs)},
                )
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
        validate_unique_step_ids(selected, "visible selected trajectories")

    dropped_cursor_rows: list[dict[str, Any]] = []
    if args.require_visible_target_cursors:
        cursor_visible_selected = []
        for item in selected:
            missing_cursors = missing_visible_target_cursors(item)
            if missing_cursors:
                mark_debug_drop(
                    debug_audit,
                    item,
                    "non_visible_target_cursors",
                    {"missing_cursors": missing_cursors},
                )
                dropped_cursor_rows.append(
                    {
                        "step_id": item.get("step_id"),
                        "run_id": item.get("run_id"),
                        "mode": item.get("mode"),
                        "tool": item.get("target_action", {}).get("tool"),
                        "action_type": item.get("target_action", {}).get("type"),
                        "missing_cursors": missing_cursors,
                    }
                )
            else:
                cursor_visible_selected.append(item)
        selected = cursor_visible_selected
        validate_unique_step_ids(selected, "cursor visible selected trajectories")

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
    mark_debug_split(debug_audit, train_trajectories, "train")
    mark_debug_split(debug_audit, eval_trajectories, "eval")
    debug_audit_rows = [
        debug_audit[item["step_id"]]
        for item in source_trajectories
        if item.get("step_id") in debug_audit
    ]

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
    write_jsonl(
        args.output / "dropped_non_visible_target_cursors.jsonl",
        dropped_cursor_rows,
    )
    write_jsonl(args.output / "debug_audit.jsonl", debug_audit_rows)
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
        "debug_audit": str(args.output / "debug_audit.jsonl"),
        "seed": args.seed,
        "max_refs": args.max_refs,
        "include_modes": sorted(set(args.include_mode)),
        "exclude_modes": sorted(set(args.exclude_mode)),
        "anonymize_refs": args.anonymize_refs,
        "require_visible_target_refs": args.require_visible_target_refs,
        "require_visible_target_cursors": args.require_visible_target_cursors,
        "inject_target_request_fields": args.inject_target_request_fields,
        "dropped_non_visible_target_refs": len(dropped_rows),
        "dropped_non_visible_target_cursors": len(dropped_cursor_rows),
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


def filter_by_mode(
    rows: list[dict[str, Any]], include_modes: list[str], exclude_modes: list[str]
) -> list[dict[str, Any]]:
    include = {mode for mode in include_modes if mode}
    exclude = {mode for mode in exclude_modes if mode}
    if not include and not exclude:
        return rows
    filtered = []
    for row in rows:
        mode = row.get("mode")
        if include and mode not in include:
            continue
        if exclude and mode in exclude:
            continue
        filtered.append(row)
    return filtered


def build_pair(
    item: dict[str, Any],
    max_refs: int,
    anonymize_refs: bool,
) -> tuple[dict[str, Any], dict[str, Any], dict[str, Any]]:
    model_item = sanitize_model_facing_trajectory(item)
    add_operator_state_features(model_item)
    assert_model_facing_visible_state_clean(model_item)
    if anonymize_refs:
        model_item = anonymize_trajectory(model_item)
        assert_model_facing_visible_state_clean(model_item)
    return to_sft_row(model_item, max_refs), item, model_item


def validate_unique_step_ids(rows: list[dict[str, Any]], label: str) -> None:
    seen: dict[str, int] = {}
    for index, row in enumerate(rows, start=1):
        step_id = row.get("step_id")
        if not isinstance(step_id, str) or not step_id:
            raise ValueError(f"{label} row {index} missing string step_id")
        previous = seen.get(step_id)
        if previous is not None:
            raise ValueError(
                f"{label} duplicate step_id `{step_id}` at rows {previous} and {index}; "
                "operator SFT data requires unique decision ids"
            )
        seen[step_id] = index


def split_pairs(
    pairs: list[tuple[dict[str, Any], dict[str, Any], dict[str, Any]]],
    args: argparse.Namespace,
) -> tuple[
    list[tuple[dict[str, Any], dict[str, Any], dict[str, Any]]],
    list[tuple[dict[str, Any], dict[str, Any], dict[str, Any]]],
    dict[str, Any],
]:
    eval_group_values = requested_eval_group_values(args)
    required_capabilities = profile_required_capabilities(args.capability_split_profile)
    if eval_group_values and args.split_mode != "group":
        raise ValueError("--eval-group-values requires --split-mode=group")
    if args.split_mode == "row":
        shuffled = list(pairs)
        stable_shuffle(shuffled, args.seed)
        eval_size = int(round(len(shuffled) * args.eval_ratio))
        if len(shuffled) > 1 and args.eval_ratio > 0:
            eval_size = max(1, eval_size)
        return shuffled[eval_size:], shuffled[:eval_size], {
            "split_mode": "row",
            "group_key": None,
            "train_groups": None,
            "eval_groups": None,
            "shuffle_strategy": "stable_fnv64_v1",
        }

    groups: dict[str, list[tuple[dict[str, Any], dict[str, Any], dict[str, Any]]]] = {}
    for pair in pairs:
        group = group_value(pair[1], args.group_key)
        groups.setdefault(group, []).append(pair)
    group_ids = sorted(groups)
    validate_capability_group_counts(groups, required_capabilities, args)
    group_capability_counts_by_group = group_capability_count_map(
        groups, required_capabilities
    )
    total_capability_counts = aggregate_capability_counts(
        group_ids, group_capability_counts_by_group
    )
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
        return finalize_group_split(
            groups,
            train_group_ids,
            eval_group_ids,
            args,
            required_capabilities,
            "explicit",
        )

    ordered_group_ids = list(group_ids)
    stable_shuffle(ordered_group_ids, args.seed)
    target_eval_rows = int(round(len(pairs) * args.eval_ratio))
    if len(pairs) > 1 and args.eval_ratio > 0:
        target_eval_rows = max(1, target_eval_rows)

    if required_capabilities:
        eval_group_ids = capability_eval_group_ids(
            groups,
            ordered_group_ids,
            required_capabilities,
            target_eval_rows,
            args.require_train_capability_coverage,
            group_capability_counts_by_group,
            total_capability_counts,
        )
        eval_group_selection = f"capability:{args.capability_split_profile}"
    else:
        eval_group_ids = []
        eval_group_selection = "ratio"

    eval_group_set = set(eval_group_ids)
    eval_rows = sum(len(groups[group_id]) for group_id in eval_group_ids)
    eval_capability_counts = aggregate_capability_counts(
        eval_group_ids, group_capability_counts_by_group
    )
    for group_id in ordered_group_ids:
        if group_id in eval_group_set:
            continue
        if (
            required_capabilities
            and args.require_train_capability_coverage
            and not train_covers_after_eval_counts(
                total_capability_counts,
                merged_capability_counts(
                    eval_capability_counts,
                    group_capability_counts_by_group[group_id],
                ),
                required_capabilities,
            )
        ):
            continue
        if eval_rows < target_eval_rows:
            eval_group_ids.append(group_id)
            eval_group_set.add(group_id)
            eval_rows += len(groups[group_id])
            add_capability_counts(
                eval_capability_counts, group_capability_counts_by_group[group_id]
            )

    if len(eval_group_ids) == len(group_ids) and eval_group_ids:
        moved_group = eval_group_ids.pop()
        eval_group_set.remove(moved_group)

    train_group_ids = [
        group_id for group_id in ordered_group_ids if group_id not in eval_group_set
    ]
    if not eval_group_ids:
        raise ValueError("group split produced an empty eval split")
    if not train_group_ids:
        raise ValueError("group split produced an empty train split")

    return finalize_group_split(
        groups,
        train_group_ids,
        eval_group_ids,
        args,
        required_capabilities,
        eval_group_selection,
    )


def stable_shuffle(values: list[Any], seed: int) -> None:
    values.sort(key=lambda value: stable_shuffle_key(value, seed))


def stable_shuffle_key(value: Any, seed: int) -> tuple[int, str]:
    payload = json.dumps(
        value,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
    )
    state = (FNV64_OFFSET ^ (seed & FNV64_MASK)) & FNV64_MASK
    for byte in payload.encode("utf-8"):
        state ^= byte
        state = (state * FNV64_PRIME) & FNV64_MASK
    return state, payload


def profile_required_capabilities(profile: str | None) -> tuple[str, ...]:
    if profile == "read":
        return READ_REQUIRED_CAPABILITIES
    if profile == "full":
        return FULL_REQUIRED_CAPABILITIES
    return ()


def validate_capability_group_counts(
    groups: dict[str, list[tuple[dict[str, Any], dict[str, Any], dict[str, Any]]]],
    required_capabilities: tuple[str, ...],
    args: argparse.Namespace,
) -> None:
    if not required_capabilities:
        return
    group_counts = capability_group_counts(groups, required_capabilities)
    missing_from_dataset = [
        capability
        for capability in required_capabilities
        if group_counts.get(capability, 0) == 0
    ]
    if missing_from_dataset:
        raise ValueError(
            "dataset does not contain required capabilities for "
            f"{args.capability_split_profile}: "
            + ", ".join(missing_from_dataset)
        )
    if args.require_train_capability_coverage and args.require_eval_capability_coverage:
        undersupplied = [
            capability
            for capability in required_capabilities
            if group_counts.get(capability, 0) < 2
        ]
        if undersupplied:
            details = ", ".join(
                f"{capability}({group_counts.get(capability, 0)} group)"
                for capability in undersupplied
            )
            raise ValueError(
                "capability-aware train+eval coverage requires each required "
                "capability to appear in at least two distinct groups; "
                f"undersupplied: {details}"
            )


def capability_group_counts(
    groups: dict[str, list[tuple[dict[str, Any], dict[str, Any], dict[str, Any]]]],
    required_capabilities: tuple[str, ...],
) -> dict[str, int]:
    counts = {capability: 0 for capability in required_capabilities}
    required = set(required_capabilities)
    for pairs in groups.values():
        group_capabilities = observed_capability_counts(pairs).keys()
        for capability in required.intersection(group_capabilities):
            counts[capability] += 1
    return counts


def capability_eval_group_ids(
    groups: dict[str, list[tuple[dict[str, Any], dict[str, Any], dict[str, Any]]]],
    ordered_group_ids: list[str],
    required_capabilities: tuple[str, ...],
    target_eval_rows: int,
    preserve_train_coverage: bool,
    group_capability_counts_by_group: dict[str, dict[str, int]],
    total_capability_counts: dict[str, int],
) -> list[str]:
    required = set(required_capabilities)
    group_capabilities = {
        group_id: set(group_capability_counts_by_group[group_id].keys()) & required
        for group_id in ordered_group_ids
    }
    selected: list[str] = []
    selected_set: set[str] = set()
    covered: set[str] = set()
    order_index = {group_id: index for index, group_id in enumerate(ordered_group_ids)}
    eval_capability_counts: dict[str, int] = {}

    while covered != required:
        candidates = []
        missing = required - covered
        for group_id in ordered_group_ids:
            if group_id in selected_set:
                continue
            candidate_eval_counts = merged_capability_counts(
                eval_capability_counts,
                group_capability_counts_by_group[group_id],
            )
            if preserve_train_coverage and not train_covers_after_eval_counts(
                total_capability_counts, candidate_eval_counts, required_capabilities
            ):
                continue
            gained = group_capabilities[group_id] & missing
            if gained:
                candidates.append(
                    (
                        -len(gained),
                        len(groups[group_id]),
                        order_index[group_id],
                        group_id,
                    )
                )
        if not candidates:
            break
        _, _, _, selected_group = sorted(candidates)[0]
        selected.append(selected_group)
        selected_set.add(selected_group)
        add_capability_counts(
            eval_capability_counts, group_capability_counts_by_group[selected_group]
        )
        covered.update(group_capabilities[selected_group])

    # Keep capability coverage mandatory, then let --eval-ratio fill extra groups.
    # This intentionally allows eval to exceed the ratio when rare contract
    # capabilities live in larger groups.
    if sum(len(groups[group_id]) for group_id in selected) < target_eval_rows:
        return selected
    return selected


def group_capability_count_map(
    groups: dict[str, list[tuple[dict[str, Any], dict[str, Any], dict[str, Any]]]],
    required_capabilities: tuple[str, ...],
) -> dict[str, dict[str, int]]:
    required = set(required_capabilities)
    return {
        group_id: {
            capability: count
            for capability, count in observed_capability_counts(pairs).items()
            if capability in required
        }
        for group_id, pairs in groups.items()
    }


def aggregate_capability_counts(
    group_ids: list[str],
    group_capability_counts_by_group: dict[str, dict[str, int]],
) -> dict[str, int]:
    counts: dict[str, int] = {}
    for group_id in group_ids:
        add_capability_counts(counts, group_capability_counts_by_group[group_id])
    return counts


def merged_capability_counts(
    left: dict[str, int], right: dict[str, int]
) -> dict[str, int]:
    merged = dict(left)
    add_capability_counts(merged, right)
    return merged


def add_capability_counts(target: dict[str, int], source: dict[str, int]) -> None:
    for capability, count in source.items():
        target[capability] = target.get(capability, 0) + count


def train_covers_after_eval_counts(
    total_capability_counts: dict[str, int],
    eval_capability_counts: dict[str, int],
    required_capabilities: tuple[str, ...],
) -> bool:
    return all(
        total_capability_counts.get(capability, 0)
        - eval_capability_counts.get(capability, 0)
        > 0
        for capability in required_capabilities
    )


def finalize_group_split(
    groups: dict[str, list[tuple[dict[str, Any], dict[str, Any], dict[str, Any]]]],
    train_group_ids: list[str],
    eval_group_ids: list[str],
    args: argparse.Namespace,
    required_capabilities: tuple[str, ...],
    selection: str,
) -> tuple[
    list[tuple[dict[str, Any], dict[str, Any], dict[str, Any]]],
    list[tuple[dict[str, Any], dict[str, Any], dict[str, Any]]],
    dict[str, Any],
]:
    train_pairs = [pair for group_id in train_group_ids for pair in groups[group_id]]
    eval_pairs = [pair for group_id in eval_group_ids for pair in groups[group_id]]
    summary: dict[str, Any] = {
        "split_mode": "group",
        "group_key": args.group_key,
        "train_groups": len(train_group_ids),
        "eval_groups": len(eval_group_ids),
        "eval_group_selection": selection,
        "eval_group_values": eval_group_ids,
        "shuffle_strategy": "stable_fnv64_v1",
    }
    if required_capabilities:
        summary["capability_split_profile"] = args.capability_split_profile
        summary["all_capability_coverage"] = capability_coverage_summary(
            required_capabilities, train_pairs + eval_pairs
        )
        summary["train_capability_coverage"] = capability_coverage_summary(
            required_capabilities, train_pairs
        )
        summary["eval_capability_coverage"] = capability_coverage_summary(
            required_capabilities, eval_pairs
        )
        summary["capability_group_counts"] = capability_group_counts(
            groups, required_capabilities
        )
        enforce_capability_requirements(summary, args)
    return train_pairs, eval_pairs, summary


def capability_coverage_summary(
    required_capabilities: tuple[str, ...],
    pairs: list[tuple[dict[str, Any], dict[str, Any], dict[str, Any]]],
) -> dict[str, Any]:
    counts = observed_capability_counts(pairs)
    missing = [
        capability
        for capability in required_capabilities
        if counts.get(capability, 0) == 0
    ]
    covered = len(required_capabilities) - len(missing)
    total = len(required_capabilities)
    percent = 100.0 if total == 0 else covered * 100.0 / total
    return {
        "covered": covered,
        "total": total,
        "percent": round(percent, 4),
        "missing": missing,
        "observed_required_counts": {
            capability: counts.get(capability, 0)
            for capability in required_capabilities
            if counts.get(capability, 0) > 0
        },
    }


def enforce_capability_requirements(summary: dict[str, Any], args: argparse.Namespace) -> None:
    all_missing = summary["all_capability_coverage"]["missing"]
    if all_missing:
        raise ValueError(
            "selected dataset lacks required capabilities for "
            f"{args.capability_split_profile}: "
            + ", ".join(all_missing)
        )
    if args.require_train_capability_coverage:
        train_missing = summary["train_capability_coverage"]["missing"]
        if train_missing:
            raise ValueError(
                "train split lacks required capabilities for "
                f"{args.capability_split_profile}: "
                + ", ".join(train_missing)
            )
    if args.require_eval_capability_coverage:
        eval_missing = summary["eval_capability_coverage"]["missing"]
        if eval_missing:
            raise ValueError(
                "eval split lacks required capabilities for "
                f"{args.capability_split_profile}: "
                + ", ".join(eval_missing)
            )


def observed_capability_counts(
    pairs: list[tuple[dict[str, Any], dict[str, Any], dict[str, Any]]],
) -> dict[str, int]:
    counts: dict[str, int] = {}
    for _, trajectory, _ in pairs:
        for capability in action_capabilities(trajectory.get("target_action", {})):
            counts[capability] = counts.get(capability, 0) + 1
    return dict(sorted(counts.items()))


def action_capabilities(action: dict[str, Any]) -> set[str]:
    capabilities: set[str] = set()
    action_type = action.get("type")
    if action_type == "stop":
        capabilities.add("tool:stop")
        capabilities.add("window:stop_sufficient")
        return capabilities
    if action_type != "tool_call":
        return capabilities

    tool = action.get("tool")
    if isinstance(tool, str):
        tool_capability = tool_capability_id(tool)
        if tool_capability is not None:
            capabilities.add(tool_capability)
    arguments = action.get("arguments")
    if not isinstance(arguments, dict):
        return capabilities

    capabilities.update(dimension_capabilities(arguments))
    if tool == "kernel_near":
        capabilities.update(cursor_capabilities(arguments.get("around")))
    elif tool == "kernel_goto":
        capabilities.update(cursor_capabilities(arguments.get("at")))
    elif tool in {"kernel_rewind", "kernel_forward"}:
        capabilities.update(cursor_capabilities(arguments.get("from")))
    elif tool == "kernel_trace":
        page = arguments.get("page")
        if isinstance(page, dict):
            capabilities.add(
                "trace.page:continue" if "cursor" in page else "trace.page:first"
            )
    elif tool == "kernel_inspect":
        include = arguments.get("include")
        if isinstance(include, dict) and include.get("raw") is False:
            capabilities.add("inspect.raw:false")
    elif tool == "kernel_write_memory":
        capabilities.update(write_memory_capabilities(arguments))

    capabilities.update(window_capabilities(arguments))
    return capabilities


def tool_capability_id(tool: str) -> str | None:
    if tool in {
        "kernel_wake",
        "kernel_ask",
        "kernel_near",
        "kernel_goto",
        "kernel_rewind",
        "kernel_forward",
        "kernel_trace",
        "kernel_inspect",
        "kernel_ingest",
        "kernel_write_memory",
    }:
        return f"tool:{tool}"
    return None


def dimension_capabilities(arguments: dict[str, Any]) -> set[str]:
    dimensions = arguments.get("dimensions")
    if not isinstance(dimensions, dict):
        return set()
    capabilities: set[str] = set()
    mode = dimensions.get("mode")
    if mode in {"all", "only", "except"}:
        capabilities.add(f"dimensions.mode:{mode}")
    scope = dimensions.get("scope")
    if scope in {"current_about", "abouts", "all_abouts"}:
        capabilities.add(f"dimensions.scope:{scope}")
    return capabilities


def cursor_capabilities(cursor: Any) -> set[str]:
    if not isinstance(cursor, dict):
        return set()
    if "ref" in cursor:
        return {"cursor:ref"}
    if "time" in cursor:
        return {"cursor:time"}
    if "sequence" in cursor:
        return {"cursor:sequence"}
    return set()


def window_capabilities(arguments: dict[str, Any]) -> set[str]:
    capabilities: set[str] = set()
    before = nested_number(arguments, "window", "before_entries")
    entries = nested_number(arguments, "limit", "entries")
    if (before is not None and before > 6) or (entries is not None and entries > 12):
        capabilities.add("window:expand")
    if (before is not None and before < 6) or (entries is not None and entries < 12):
        capabilities.add("window:shrink")
    return capabilities


def nested_number(value: dict[str, Any], parent: str, child: str) -> int | float | None:
    parent_value = value.get(parent)
    if not isinstance(parent_value, dict):
        return None
    number = parent_value.get(child)
    if isinstance(number, (int, float)) and not isinstance(number, bool):
        return number
    return None


def write_memory_capabilities(arguments: dict[str, Any]) -> set[str]:
    capabilities: set[str] = set()
    connect_to = arguments.get("connect_to")
    if not isinstance(connect_to, list):
        return capabilities

    relation_refs = [
        link.get("ref")
        for link in connect_to
        if isinstance(link, dict) and isinstance(link.get("ref"), str)
    ]
    if connect_to and all(rich_relation_link(link) for link in connect_to):
        capabilities.add("write:relation_quality")

    read_context_refs = write_read_context_refs(arguments.get("read_context"))
    if any(ref in read_context_refs for ref in relation_refs):
        capabilities.add("write:read_context_proof")
    return capabilities


def rich_relation_link(value: Any) -> bool:
    if not isinstance(value, dict):
        return False
    for field in ("ref", "rel", "class", "why", "evidence"):
        field_value = value.get(field)
        if not isinstance(field_value, str) or not field_value:
            return False
    return True


def write_read_context_refs(read_context: Any) -> set[str]:
    if not isinstance(read_context, dict):
        return set()
    refs: set[str] = set()
    for field in ("inspected_refs", "temporal_refs", "wake_refs", "ask_refs"):
        values = read_context.get(field)
        if isinstance(values, list):
            refs.update(value for value in values if isinstance(value, str))
    trace_paths = read_context.get("trace_paths")
    if isinstance(trace_paths, list):
        for path in trace_paths:
            if not isinstance(path, dict):
                continue
            for field in ("from", "to"):
                value = path.get(field)
                if isinstance(value, str):
                    refs.add(value)
            values = path.get("refs")
            if isinstance(values, list):
                refs.update(value for value in values if isinstance(value, str))
    return refs


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
    task_id = trajectory_task_id(item)
    if task_id is not None:
        return str(task_id)
    about = str(item.get("about", "missing"))
    marker = ":task:"
    if marker in about:
        return about.rsplit(marker, 1)[-1].split(":", 1)[0]
    if key == "task_or_step":
        return str(item.get("step_id", about))
    return about


def trajectory_task_id(item: dict[str, Any]) -> Any:
    task_id = (
        item.get("visible_state", {})
        .get("task", {})
        .get("task_id")
    )
    if task_id is not None:
        return task_id
    question_id = (
        item.get("visible_state", {})
        .get("task", {})
        .get("question_id")
    )
    if question_id is not None:
        return question_id
    writer_question_id = (
        item.get("visible_state", {})
        .get("writer", {})
        .get("question_id")
    )
    if writer_question_id is not None:
        return writer_question_id
    return None


def to_sft_row(item: dict[str, Any], max_refs: int) -> dict[str, Any]:
    visible_state = compact_visible_state(item["visible_state"], max_refs)
    assert_model_facing_visible_state_clean(
        {"step_id": item["step_id"], "visible_state": visible_state}
    )
    user_payload = {
        "task_family": item["task_family"],
        "mode": item["mode"],
        "about": item["about"],
        "goal": item.get("goal"),
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
    abouts: dict[str, str] = {}
    about_values = collect_about_values(item)

    def about_id(value: str) -> str:
        if value not in abouts:
            abouts[value] = f"about_{len(abouts) + 1:04d}"
        return abouts[value]

    def ref_id(value: str) -> str:
        if value not in refs:
            refs[value] = f"ref_{len(refs) + 1:04d}"
        return refs[value]

    def rewrite(value: Any) -> Any:
        if isinstance(value, str):
            if value in about_values:
                return about_id(value)
            if looks_like_ref(value):
                return ref_id(value)
            return value
        if isinstance(value, list):
            return [rewrite(item) for item in value]
        if isinstance(value, dict):
            return {key: rewrite(child) for key, child in value.items()}
        return value

    anonymized = json.loads(json.dumps(item))
    if isinstance(anonymized.get("about"), str):
        anonymized["about"] = rewrite(anonymized["about"])
    for key in ("visible_state", "target_action", "observed_outcome", "quality"):
        if key in anonymized:
            anonymized[key] = rewrite(anonymized[key])
    anonymized["ref_anonymization"] = {
        "mode": "per_trajectory",
        "ref_count": len(refs),
        "about_count": len(abouts),
    }
    return anonymized


def collect_about_values(item: dict[str, Any]) -> set[str]:
    values: set[str] = set()
    about = item.get("about")
    if isinstance(about, str) and about:
        values.add(about)

    def walk(value: Any, key: str | None = None) -> None:
        if isinstance(value, dict):
            for child_key, child in value.items():
                if child_key == "about" and isinstance(child, str) and child:
                    values.add(child)
                elif child_key == "abouts" and isinstance(child, list):
                    values.update(item for item in child if isinstance(item, str) and item)
                walk(child, child_key)
            return
        if isinstance(value, list):
            for child in value:
                walk(child, key)

    walk(item)
    return values


def looks_like_ref(value: str) -> bool:
    return (
        value.startswith("memoryarena:run:")
        or value.startswith("longmemeval:")
        or value.startswith("memoryagentbench:")
        or value.startswith("incident:")
        or value.startswith("about:")
        or value.startswith("evidence:")
        or value.startswith("question:run:")
        or value.startswith("turn:run:")
        or ":subtask:" in value
        or ":task:" in value
    )


def build_debug_audit_row(item: dict[str, Any]) -> dict[str, Any]:
    action = item.get("target_action", {})
    visible_cursors = visible_cursor_values(item.get("visible_state", {}))
    return {
        "step_id": item.get("step_id"),
        "run_id": item.get("run_id"),
        "source": item.get("source"),
        "mode": item.get("mode"),
        "task_family": item.get("task_family"),
        "about": item.get("about"),
        "goal": item.get("goal"),
        "status": "candidate",
        "split": None,
        "drop_reasons": [],
        "target": {
            "action_type": action.get("type"),
            "tool": action.get("tool"),
            "argument_keys": target_argument_keys(action),
            "capabilities": sorted(action_capabilities(action)),
            "refs": sorted(target_primary_refs(item)),
            "cursors": target_cursor_values(item),
        },
        "visibility": {
            "visible_refs_count": len(visible_refs(item)),
            "visible_refs": sorted(visible_refs(item)),
            "missing_refs": sorted(missing_visible_target_refs(item)),
            "visible_cursors": {
                key: sorted(values)
                for key, values in sorted(visible_cursors.items())
            },
            "missing_cursors": missing_visible_target_cursors(item),
        },
        "state": {
            "last_tool": item.get("visible_state", {}).get("last_tool")
            if isinstance(item.get("visible_state"), dict)
            else None,
            "remaining_budget": item.get("visible_state", {}).get("remaining_budget")
            if isinstance(item.get("visible_state"), dict)
            else None,
        },
    }


def target_argument_keys(action: dict[str, Any]) -> list[str]:
    args = action.get("arguments")
    if not isinstance(args, dict):
        return []
    return sorted(args)


def target_cursor_values(item: dict[str, Any]) -> list[dict[str, str]]:
    action = item.get("target_action", {})
    if action.get("type") != "tool_call":
        return []
    tool = action.get("tool")
    args = action.get("arguments")
    if not isinstance(args, dict):
        return []

    cursors: list[dict[str, str]] = []
    if tool == "kernel_near":
        record_target_cursor(cursors, "arguments.around", args.get("around"))
    elif tool == "kernel_goto":
        record_target_cursor(cursors, "arguments.at", args.get("at"))
    elif tool in {"kernel_rewind", "kernel_forward"}:
        record_target_cursor(cursors, "arguments.from", args.get("from"))
    elif tool == "kernel_trace":
        page = args.get("page")
        if isinstance(page, dict) and isinstance(page.get("cursor"), str):
            cursors.append(
                {
                    "path": "arguments.page.cursor",
                    "kind": "page_cursor",
                    "value": page["cursor"],
                }
            )
    return cursors


def record_target_cursor(
    cursors: list[dict[str, str]],
    path: str,
    cursor: Any,
) -> None:
    if not isinstance(cursor, dict):
        return
    for kind in ("time", "sequence"):
        if kind in cursor:
            cursors.append(
                {
                    "path": f"{path}.{kind}",
                    "kind": kind,
                    "value": str(cursor[kind]),
                }
            )


def mark_debug_drop(
    debug_audit: dict[str, dict[str, Any]],
    item: dict[str, Any],
    reason: str,
    details: dict[str, Any],
) -> None:
    row = debug_audit.get(item.get("step_id"))
    if row is None:
        return
    row["status"] = "dropped"
    row["drop_reasons"].append({"reason": reason, **details})


def mark_debug_split(
    debug_audit: dict[str, dict[str, Any]],
    trajectories: list[dict[str, Any]],
    split: str,
) -> None:
    for item in trajectories:
        row = debug_audit.get(item.get("step_id"))
        if row is None:
            continue
        row["status"] = "kept"
        row["split"] = split


def missing_visible_target_refs(item: dict[str, Any]) -> set[str]:
    visible = visible_refs(item)
    return {ref for ref in target_primary_refs(item) if ref not in visible}


def missing_visible_target_cursors(item: dict[str, Any]) -> list[dict[str, str]]:
    visible = visible_cursor_values(item.get("visible_state", {}))
    action = item.get("target_action", {})
    if action.get("type") != "tool_call":
        return []

    tool = action.get("tool")
    args = action.get("arguments")
    if not isinstance(args, dict):
        return []

    missing: list[dict[str, str]] = []
    if tool == "kernel_near":
        record_missing_cursor(missing, "arguments.around", args.get("around"), visible)
    elif tool == "kernel_goto":
        record_missing_cursor(missing, "arguments.at", args.get("at"), visible)
    elif tool in {"kernel_rewind", "kernel_forward"}:
        record_missing_cursor(missing, "arguments.from", args.get("from"), visible)
    elif tool == "kernel_trace":
        page = args.get("page")
        if isinstance(page, dict) and isinstance(page.get("cursor"), str):
            cursor = page["cursor"]
            if cursor not in visible["page_cursor"]:
                missing.append(
                    {
                        "path": "arguments.page.cursor",
                        "kind": "page_cursor",
                        "value": cursor,
                    }
                )
    return missing


def record_missing_cursor(
    missing: list[dict[str, str]],
    path: str,
    cursor: Any,
    visible: dict[str, set[str]],
) -> None:
    if not isinstance(cursor, dict):
        return
    for kind in ("time", "sequence"):
        if kind not in cursor:
            continue
        value = str(cursor[kind])
        if value not in visible[kind]:
            missing.append({"path": f"{path}.{kind}", "kind": kind, "value": value})


def visible_cursor_values(state: Any) -> dict[str, set[str]]:
    values = {"time": set(), "sequence": set(), "page_cursor": set()}

    def walk(value: Any, key: str | None = None) -> None:
        if isinstance(value, dict):
            for child_key, child in value.items():
                walk(child, child_key)
            return
        if isinstance(value, list):
            for child in value:
                walk(child, key)
            return
        if value is None or key is None:
            return
        normalized = str(value)
        if key == "time" or key.endswith("_time"):
            values["time"].add(normalized)
        elif key == "sequence" or key.endswith("_sequence"):
            values["sequence"].add(normalized)
        elif key in {"cursor", "next_cursor", "page_cursor"} or key.endswith("_cursor"):
            values["page_cursor"].add(normalized)

    walk(state)
    return values


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
    read_context = state.get("read_context")
    if isinstance(read_context, dict):
        for key in ("inspected_refs", "temporal_refs", "wake_refs", "ask_refs"):
            values = read_context.get(key)
            if isinstance(values, list):
                refs.update(value for value in values if isinstance(value, str))
        trace_paths = read_context.get("trace_paths")
        if isinstance(trace_paths, list):
            for path in trace_paths:
                if not isinstance(path, dict):
                    continue
                for key in ("from", "to"):
                    value = path.get(key)
                    if isinstance(value, str):
                        refs.add(value)
                values = path.get("refs")
                if isinstance(values, list):
                    refs.update(value for value in values if isinstance(value, str))
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
    for cursor_key in ("at", "from"):
        cursor = args.get(cursor_key)
        if isinstance(cursor, dict) and isinstance(cursor.get("ref"), str):
            refs.add(cursor["ref"])
    connect_to = args.get("connect_to")
    if isinstance(connect_to, list):
        for link in connect_to:
            if isinstance(link, dict) and isinstance(link.get("ref"), str):
                refs.add(link["ref"])
    memory = args.get("memory")
    if isinstance(memory, dict):
        relations = memory.get("relations")
        if isinstance(relations, list):
            for relation in relations:
                if not isinstance(relation, dict):
                    continue
                for key in ("from", "to"):
                    value = relation.get(key)
                    if isinstance(value, str):
                        refs.add(value)
    return refs


def compact_visible_state(value: dict[str, Any], max_refs: int) -> dict[str, Any]:
    compact = json.loads(json.dumps(value))
    for key in ("candidate_refs", "candidate_ref_details", "known_refs", "last_observed_refs"):
        refs = compact.get(key)
        if isinstance(refs, list) and len(refs) > max_refs:
            compact[f"{key}_truncated"] = len(refs) - max_refs
            compact[key] = refs[:max_refs]
    return compact


def sanitize_model_facing_trajectory(item: dict[str, Any]) -> dict[str, Any]:
    """Remove exporter-only metadata from the model-facing trajectory.

    Audit trajectories may keep provenance fields for debugging. SFT prompts must
    expose only the state a real MCP/KMP operator would see.
    """
    model_item = json.loads(json.dumps(item))
    state = model_item.get("visible_state")
    if not isinstance(state, dict):
        return model_item

    state.pop("writer", None)
    candidate_details = state.get("candidate_ref_details")
    if isinstance(candidate_details, list):
        for detail in candidate_details:
            if isinstance(detail, dict):
                detail.pop("sources", None)
    return model_item


def add_operator_state_features(item: dict[str, Any]) -> None:
    """Add compact, non-gold state features for small operator models.

    These fields are derived only from visible state. They do not reveal the
    target action; they make already-visible navigation state easier for small
    models to consume without counting long arrays or inferring phases from
    raw tool history.
    """
    state = item.get("visible_state")
    if not isinstance(state, dict):
        return

    candidate_details = state.get("candidate_ref_details")
    if not isinstance(candidate_details, list):
        candidate_details = []
    candidate_refs = state.get("candidate_refs")
    if not isinstance(candidate_refs, list):
        candidate_refs = []
    known_refs = state.get("known_refs")
    if not isinstance(known_refs, list):
        known_refs = []
    observed_refs = state.get("last_observed_refs")
    if not isinstance(observed_refs, list):
        observed_refs = []

    last_tool = state.get("last_tool")
    if not isinstance(last_tool, str):
        last_tool = None

    primary_candidate = first_primary_candidate(candidate_details)
    operator_state: dict[str, Any] = {
        "navigation_phase": navigation_phase(last_tool, len(observed_refs)),
        "last_tool": last_tool or "none",
        "last_observed_ref_count": len(observed_refs),
        "candidate_ref_count": len(candidate_refs),
        "candidate_detail_count": len(candidate_details),
        "known_ref_count": len(known_refs),
        "has_candidate_details": bool(candidate_details),
        "has_observed_refs": bool(observed_refs),
    }
    if primary_candidate is not None:
        operator_state["primary_candidate"] = primary_candidate
    state["operator_state"] = operator_state


def first_primary_candidate(candidate_details: list[Any]) -> dict[str, Any] | None:
    typed_details = [detail for detail in candidate_details if isinstance(detail, dict)]
    if not typed_details:
        return None

    def sort_key(detail: dict[str, Any]) -> tuple[int, int]:
        role = detail.get("role")
        priority = detail.get("priority")
        if not isinstance(priority, int):
            priority = 10_000
        role_rank = 0 if role in {"target_question", "current", "anchor"} else 1
        return role_rank, priority

    selected = sorted(typed_details, key=sort_key)[0]
    compact: dict[str, Any] = {}
    for key in ("ref", "role", "relation_hint", "priority"):
        value = selected.get(key)
        if value is not None:
            compact[key] = value
    return compact or None


def navigation_phase(last_tool: str | None, observed_ref_count: int) -> str:
    if last_tool is None:
        return "start"
    if last_tool == "kernel_near" and observed_ref_count > 0:
        return "after_near_with_observed_refs"
    if last_tool == "kernel_near":
        return "after_near_without_observed_refs"
    return f"after_{last_tool}"


def assert_model_facing_visible_state_clean(item: dict[str, Any]) -> None:
    state = item.get("visible_state")
    if not isinstance(state, dict):
        return

    findings: list[str] = []

    def walk(value: Any, path: str) -> None:
        if isinstance(value, dict):
            for key, child in value.items():
                child_path = f"{path}.{key}"
                if key in {"sources", "writer"}:
                    findings.append(child_path)
                walk(child, child_path)
            return
        if isinstance(value, list):
            for index, child in enumerate(value):
                walk(child, f"{path}[{index}]")
            return
        if isinstance(value, str) and is_forbidden_model_visible_string(value):
            findings.append(path)

    walk(state, "visible_state")
    if findings:
        step_id = item.get("step_id", "unknown")
        raise ValueError(
            f"model-facing visible_state contains exporter-only context for {step_id}: "
            + ", ".join(sorted(set(findings)))
        )


def is_forbidden_model_visible_string(value: str) -> bool:
    return (
        value in FORBIDDEN_MODEL_VISIBLE_STRING_VALUES
        or any(value.startswith(prefix) for prefix in FORBIDDEN_MODEL_VISIBLE_STRING_PREFIXES)
    )


def inject_target_request_fields(item: dict[str, Any]) -> dict[str, Any]:
    cloned = json.loads(json.dumps(item))
    state = cloned.setdefault("visible_state", {})
    if not isinstance(state, dict):
        return cloned

    action = cloned.get("target_action")
    if not isinstance(action, dict):
        return cloned
    action_type = action.get("type")
    if action_type == "stop":
        state["requested_stop"] = {
            key: value
            for key, value in action.items()
            if key in {"answer_policy", "final_refs", "reason"}
        }
        return cloned
    if action_type != "tool_call":
        return cloned

    tool = action.get("tool")
    arguments = action.get("arguments")
    if not isinstance(tool, str) or not isinstance(arguments, dict):
        return cloned

    if tool == "kernel_wake":
        state["requested_wake"] = without_keys(arguments, {"about"})
    elif tool == "kernel_ask":
        state["requested_ask"] = without_keys(arguments, {"about"})
    elif tool in {"kernel_near", "kernel_goto", "kernel_rewind", "kernel_forward"}:
        cursor_key = temporal_cursor_key(tool)
        cursor = arguments.get(cursor_key)
        if cursor is not None:
            state["requested_move"] = {
                "kind": tool,
                "cursor_key": cursor_key,
                "cursor": cursor,
            }
        dimensions = arguments.get("dimensions")
        if isinstance(dimensions, dict):
            state["requested_scope"] = dimensions
        bounds = {
            key: arguments[key]
            for key in ("include", "limit", "window", "budget")
            if key in arguments
        }
        if bounds:
            state["requested_bounds"] = bounds
    elif tool == "kernel_trace":
        state["requested_trace"] = dict(arguments)
    elif tool == "kernel_inspect":
        state["inspection_request"] = dict(arguments)
    return cloned


def without_keys(value: dict[str, Any], keys: set[str]) -> dict[str, Any]:
    return {key: child for key, child in value.items() if key not in keys}


def temporal_cursor_key(tool: str) -> str:
    if tool == "kernel_near":
        return "around"
    if tool == "kernel_goto":
        return "at"
    return "from"


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
