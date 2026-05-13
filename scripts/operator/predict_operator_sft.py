#!/usr/bin/env python3
"""Generate evaluator-compatible predictions from a trained KMP operator."""

from __future__ import annotations

import argparse
import json
import shutil
import sys
from pathlib import Path
from typing import Any


ACTION_VALIDATOR = "kernel-operator-action-contract-v1"
SCHEMA_MODE = "strict-no-additional-properties"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Predict KMP operator actions.")
    parser.add_argument("--dataset-jsonl", required=True, type=Path)
    parser.add_argument("--model-id", required=True)
    parser.add_argument("--adapter", default=None)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--trust-remote-code", action="store_true")
    parser.add_argument(
        "--torch-dtype",
        choices=["auto", "float16", "bfloat16", "float32"],
        default="auto",
    )
    parser.add_argument(
        "--config-override",
        action="append",
        default=[],
        metavar="KEY=VALUE",
        help="Override a model config attribute before loading the model.",
    )
    parser.add_argument("--limit", type=int, default=None)
    parser.add_argument("--batch-size", type=int, default=8)
    parser.add_argument("--max-new-tokens", type=int, default=350)
    parser.add_argument("--temperature", type=float, default=0.0)
    parser.add_argument(
        "--stop-after-json",
        action="store_true",
        help=(
            "Force EOS after the first complete top-level JSON object is "
            "generated. The parser remains strict and rejects invalid actions."
        ),
    )
    parser.add_argument("--force", action="store_true")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    if args.batch_size < 1:
        raise SystemExit("--batch-size must be >= 1")
    if args.output.exists():
        if not args.force:
            raise SystemExit(f"output already exists: {args.output}; pass --force")
        shutil.rmtree(args.output)
    args.output.mkdir(parents=True)

    try:
        import torch
        from peft import PeftModel
        from transformers import (
            AutoConfig,
            AutoModelForCausalLM,
            AutoTokenizer,
            LogitsProcessorList,
        )
    except ImportError as exc:
        raise SystemExit(
            "Missing inference dependencies. Install torch, transformers, peft, "
            "and accelerate in the inference environment."
        ) from exc

    tokenizer = AutoTokenizer.from_pretrained(
        args.model_id,
        use_fast=True,
        trust_remote_code=args.trust_remote_code,
    )
    if tokenizer.pad_token is None:
        tokenizer.pad_token = tokenizer.eos_token
    tokenizer.padding_side = "left"
    config = AutoConfig.from_pretrained(
        args.model_id,
        trust_remote_code=args.trust_remote_code,
    )
    for key, value in parse_config_overrides(args.config_override).items():
        setattr(config, key, value)
    model = AutoModelForCausalLM.from_pretrained(
        args.model_id,
        config=config,
        torch_dtype=torch_dtype(args.torch_dtype, torch),
        device_map="auto",
        trust_remote_code=args.trust_remote_code,
    )
    if args.adapter:
        model = PeftModel.from_pretrained(model, args.adapter)
    model.config.pad_token_id = tokenizer.pad_token_id
    model.eval()

    rows = read_jsonl(args.dataset_jsonl)
    if args.limit is not None:
        rows = rows[: args.limit]

    predictions_path = args.output / "predictions.jsonl"
    results_path = args.output / "llm_results.jsonl"
    failures_path = args.output / "failures.jsonl"
    predictions = 0
    failures = 0
    failure_reasons: dict[str, int] = {}

    with predictions_path.open("w", encoding="utf-8") as pred_handle, results_path.open(
        "w", encoding="utf-8"
    ) as result_handle, failures_path.open("w", encoding="utf-8") as failure_handle:
        for batch_start in range(0, len(rows), args.batch_size):
            batch = rows[batch_start : batch_start + args.batch_size]
            prompts = [
                tokenizer.apply_chat_template(
                    row["messages"][:-1],
                    tokenize=False,
                    add_generation_prompt=True,
                )
                for row in batch
            ]
            inputs = tokenizer(prompts, return_tensors="pt", padding=True).to(model.device)
            prompt_width = inputs["input_ids"].shape[-1]
            with torch.inference_mode():
                generation_kwargs = {
                    "max_new_tokens": args.max_new_tokens,
                    "do_sample": args.temperature > 0,
                    "eos_token_id": tokenizer.eos_token_id,
                    "pad_token_id": tokenizer.eos_token_id,
                }
                if args.temperature > 0:
                    generation_kwargs["temperature"] = args.temperature
                if args.stop_after_json:
                    generation_kwargs["logits_processor"] = LogitsProcessorList(
                        [
                            StopAfterCompleteJsonObjectProcessor(
                                tokenizer=tokenizer,
                                prompt_width=prompt_width,
                                eos_token_id=tokenizer.eos_token_id,
                            )
                        ]
                    )
                output = model.generate(
                    **inputs,
                    **generation_kwargs,
                )

            for row, generated_output in zip(batch, output, strict=True):
                generated = generated_output[prompt_width:]
                raw = tokenizer.decode(generated, skip_special_tokens=True).strip()
                action, failure_reason = parse_action(raw)
                result = {
                    "step_id": row["step_id"],
                    "raw_response": raw,
                    "action": action,
                    "valid_action": action is not None,
                }
                if failure_reason is not None:
                    result["failure_reason"] = failure_reason
                result_handle.write(
                    json.dumps(result, separators=(",", ":"), sort_keys=True) + "\n"
                )
                if action is None:
                    failures += 1
                    reason = failure_reason or "invalid_action"
                    failure_reasons[reason] = failure_reasons.get(reason, 0) + 1
                    failure_handle.write(
                        json.dumps(
                            {
                                "step_id": row["step_id"],
                                "reason": reason,
                                "raw_response": raw,
                            },
                            separators=(",", ":"),
                            sort_keys=True,
                        )
                        + "\n"
                    )
                    continue
                pred_handle.write(
                    json.dumps(
                        {"step_id": row["step_id"], "action": action},
                        separators=(",", ":"),
                        sort_keys=True,
                    )
                    + "\n"
                )
                predictions += 1

            pred_handle.flush()
            result_handle.flush()
            failure_handle.flush()
            completed = min(batch_start + len(batch), len(rows))
            print(
                json.dumps(
                    {
                        "event": "kernel_operator_predict.progress",
                        "completed": completed,
                        "selected": len(rows),
                        "predictions": predictions,
                        "failures": failures,
                    },
                    separators=(",", ":"),
                    sort_keys=True,
                ),
                file=sys.stderr,
            )

    summary = {
        "predictor": "kernel-operator-sft-predict-v2-batched",
        "action_validator": ACTION_VALIDATOR,
        "schema_mode": SCHEMA_MODE,
        "dataset": str(args.dataset_jsonl),
        "model_id": args.model_id,
        "adapter": args.adapter,
        "selected": len(rows),
        "predictions": predictions,
        "failures": failures,
        "batch_size": args.batch_size,
        "failure_reasons": failure_reasons,
        "max_new_tokens": args.max_new_tokens,
        "stop_after_json": args.stop_after_json,
        "temperature": args.temperature,
    }
    (args.output / "summary.json").write_text(
        json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    print(json.dumps(summary, indent=2, sort_keys=True))


def read_jsonl(path: Path) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    with path.open(encoding="utf-8") as handle:
        for line in handle:
            if line.strip():
                rows.append(json.loads(line))
    return rows


def parse_config_overrides(overrides: list[str]) -> dict[str, Any]:
    parsed: dict[str, Any] = {}
    for override in overrides:
        if "=" not in override:
            raise SystemExit(f"invalid --config-override {override!r}; expected KEY=VALUE")
        key, raw_value = override.split("=", 1)
        key = key.strip()
        if not key:
            raise SystemExit(f"invalid --config-override {override!r}; empty key")
        parsed[key] = parse_config_value(raw_value.strip())
    return parsed


def parse_config_value(value: str) -> Any:
    lower = value.lower()
    if lower == "true":
        return True
    if lower == "false":
        return False
    if lower == "null":
        return None
    try:
        return int(value)
    except ValueError:
        return value


def torch_dtype(value: str, torch: Any) -> str | Any:
    if value == "auto":
        return "auto"
    return {
        "float16": torch.float16,
        "bfloat16": torch.bfloat16,
        "float32": torch.float32,
    }[value]


class StopAfterCompleteJsonObjectProcessor:
    """Force EOS once each row has produced one complete JSON object."""

    def __init__(self, tokenizer: Any, prompt_width: int, eos_token_id: int) -> None:
        self.tokenizer = tokenizer
        self.prompt_width = prompt_width
        self.eos_token_id = eos_token_id

    def __call__(self, input_ids: Any, scores: Any) -> Any:
        for row_index in range(input_ids.shape[0]):
            generated_ids = input_ids[row_index, self.prompt_width :]
            text = self.tokenizer.decode(generated_ids, skip_special_tokens=True)
            if first_complete_json_object_end(text) is not None:
                scores[row_index].fill_(-float("inf"))
                scores[row_index, self.eos_token_id] = 0
        return scores


def parse_action(raw: str) -> tuple[dict[str, Any] | None, str | None]:
    text = raw.strip()
    if not text:
        return None, "empty_response"
    end = first_complete_json_object_end(text)
    if end is None:
        return None, "incomplete_json"
    prefix = text[: text.find("{")].strip()
    suffix = text[end + 1 :].strip()
    if prefix or suffix:
        return None, "extra_content_after_json"
    try:
        value = json.loads(text)
    except json.JSONDecodeError:
        return None, "invalid_json"
    if not isinstance(value, dict):
        return None, "json_not_object"
    if set(value.keys()) != {"action"}:
        return None, "missing_or_extra_top_level_fields"
    action = value["action"]
    if not isinstance(action, dict):
        return None, "action_not_object"
    shape_error = validate_action_shape(action)
    if shape_error is not None:
        return None, shape_error
    return action, None


def first_complete_json_object_end(text: str) -> int | None:
    start = text.find("{")
    if start < 0:
        return None
    if text[:start].strip():
        return None

    depth = 0
    in_string = False
    escaped = False
    for index, char in enumerate(text[start:], start=start):
        if in_string:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                in_string = False
            continue

        if char == '"':
            in_string = True
        elif char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return index
            if depth < 0:
                return None

    return None


def validate_action_shape(action: dict[str, Any]) -> str | None:
    action_type = action.get("type")
    if action_type == "tool_call":
        key_error = exact_keys(action, {"type", "tool", "arguments"}, set(), "action")
        if key_error is not None:
            return key_error
        tool = action.get("tool")
        if not isinstance(tool, str) or not tool:
            return "missing_tool"
        arguments = action.get("arguments")
        if not isinstance(arguments, dict):
            return "missing_arguments"
        argument_error = validate_tool_arguments(tool, arguments)
        if argument_error is not None:
            return argument_error
        if not is_bounded_tool_call(tool, arguments):
            return f"unbounded_or_invalid_tool_call:{tool}"
        return None

    if action_type == "stop":
        key_error = exact_keys(
            action,
            {"type", "answer_policy", "final_refs", "reason"},
            set(),
            "action",
        )
        if key_error is not None:
            return key_error
        answer_policy_error = validate_answer_policy(action.get("answer_policy"))
        if answer_policy_error is not None:
            return answer_policy_error
        reason = action.get("reason")
        if not isinstance(reason, str) or not reason:
            return "missing_stop_reason"
        final_refs = action.get("final_refs")
        if not isinstance(final_refs, list) or not all(
            isinstance(ref, str) for ref in final_refs
        ):
            return "invalid_stop_final_refs"
        return None

    if action_type is None:
        return "missing_action_type"
    return "unsupported_action_type"


def validate_tool_arguments(tool: str, arguments: dict[str, Any]) -> str | None:
    if tool == "kernel_wake":
        key_error = exact_keys(
            arguments,
            {"about"},
            {"role", "intent", "dimensions", "depth", "budget"},
            "action.arguments",
        )
        if key_error is not None:
            return key_error
        error = require_non_empty_string(arguments, "about", "action.arguments")
        if error is not None:
            return error
        for field in ("role", "intent"):
            error = validate_optional_non_empty_string(arguments, field, "action.arguments")
            if error is not None:
                return error
        if "dimensions" in arguments:
            error = validate_dimensions(arguments["dimensions"], "action.arguments.dimensions")
            if error is not None:
                return error
        error = validate_optional_positive_int(arguments, "depth", "action.arguments")
        if error is not None:
            return error
        if "budget" in arguments:
            return validate_budget(arguments["budget"], "action.arguments.budget")
        return None

    if tool == "kernel_ask":
        key_error = exact_keys(
            arguments,
            {"about", "answer_policy", "dimensions", "question"},
            {"budget", "depth"},
            "action.arguments",
        )
        if key_error is not None:
            return key_error
        for field in ("about", "question"):
            error = require_non_empty_string(arguments, field, "action.arguments")
            if error is not None:
                return error
        error = validate_answer_policy(arguments.get("answer_policy"))
        if error is not None:
            return error
        error = validate_dimensions(arguments.get("dimensions"), "action.arguments.dimensions")
        if error is not None:
            return error
        if "budget" in arguments:
            error = validate_budget(arguments["budget"], "action.arguments.budget")
            if error is not None:
                return error
        return validate_optional_positive_int(arguments, "depth", "action.arguments")

    if tool in {"kernel_near", "kernel_goto", "kernel_rewind", "kernel_forward"}:
        cursor_key = {
            "kernel_near": "around",
            "kernel_goto": "at",
            "kernel_rewind": "from",
            "kernel_forward": "from",
        }[tool]
        return validate_temporal_arguments(arguments, cursor_key)

    if tool == "kernel_trace":
        key_error = exact_keys(
            arguments,
            {"from", "to", "budget"},
            {"goal", "role", "page"},
            "action.arguments",
        )
        if key_error is not None:
            return key_error
        for field in ("from", "to"):
            error = require_non_empty_string(arguments, field, "action.arguments")
            if error is not None:
                return error
        error = validate_budget(arguments.get("budget"), "action.arguments.budget")
        if error is not None:
            return error
        for field in ("goal", "role"):
            error = validate_optional_non_empty_string(arguments, field, "action.arguments")
            if error is not None:
                return error
        if "page" in arguments:
            return validate_page(arguments["page"], "action.arguments.page")
        return None

    if tool == "kernel_inspect":
        key_error = exact_keys(arguments, {"ref", "include"}, set(), "action.arguments")
        if key_error is not None:
            return key_error
        error = require_non_empty_string(arguments, "ref", "action.arguments")
        if error is not None:
            return error
        return validate_inspect_include(
            arguments.get("include"), "action.arguments.include"
        )

    return f"unsupported_tool:{tool}"


def validate_temporal_arguments(arguments: dict[str, Any], cursor_key: str) -> str | None:
    key_error = exact_keys(
        arguments,
        {
            "about",
            cursor_key,
            "dimensions",
            "include",
            "limit",
            "budget",
            "window",
        },
        {"depth"},
        "action.arguments",
    )
    if key_error is not None:
        return key_error
    error = require_non_empty_string(arguments, "about", "action.arguments")
    if error is not None:
        return error
    error = validate_temporal_cursor(arguments.get(cursor_key), f"action.arguments.{cursor_key}")
    if error is not None:
        return error
    error = validate_dimensions(arguments.get("dimensions"), "action.arguments.dimensions")
    if error is not None:
        return error
    error = validate_temporal_include(arguments.get("include"), "action.arguments.include")
    if error is not None:
        return error
    error = validate_limit(arguments.get("limit"), "action.arguments.limit")
    if error is not None:
        return error
    error = validate_budget(arguments.get("budget"), "action.arguments.budget")
    if error is not None:
        return error
    error = validate_window(arguments.get("window"), "action.arguments.window")
    if error is not None:
        return error
    return validate_optional_positive_int(arguments, "depth", "action.arguments")


def validate_dimensions(value: Any, context: str) -> str | None:
    if not isinstance(value, dict):
        return f"{context}_not_object"
    key_error = exact_keys(
        value,
        {"mode", "scope"},
        {"include", "exclude", "scope_ids", "abouts"},
        context,
    )
    if key_error is not None:
        return key_error
    mode = value.get("mode")
    if mode not in {"all", "only", "except"}:
        return f"{context}.mode_unsupported"
    scope = value.get("scope")
    if scope not in {"current_about", "abouts", "all_abouts"}:
        return f"{context}.scope_unsupported"
    for field in ("include", "exclude", "scope_ids", "abouts"):
        if field in value:
            error = validate_string_array(value[field], f"{context}.{field}")
            if error is not None:
                return error
    include_count = len(value.get("include", []))
    exclude_count = len(value.get("exclude", []))
    abouts_count = len(value.get("abouts", []))
    if mode == "all" and (include_count > 0 or exclude_count > 0):
        return f"{context}.mode_all_with_include_or_exclude"
    if mode == "only" and include_count == 0:
        return f"{context}.mode_only_requires_include"
    if mode == "only" and exclude_count > 0:
        return f"{context}.mode_only_with_exclude"
    if mode == "except" and exclude_count == 0:
        return f"{context}.mode_except_requires_exclude"
    if mode == "except" and include_count > 0:
        return f"{context}.mode_except_with_include"
    if scope == "current_about" and abouts_count > 0:
        return f"{context}.scope_current_about_with_abouts"
    if scope == "abouts" and abouts_count == 0:
        return f"{context}.scope_abouts_requires_abouts"
    if scope == "all_abouts" and abouts_count > 0:
        return f"{context}.scope_all_abouts_with_abouts"
    return None


def validate_temporal_cursor(value: Any, context: str) -> str | None:
    if not isinstance(value, dict):
        return f"{context}_not_object"
    key_error = exact_keys(value, set(), {"ref", "time", "sequence"}, context)
    if key_error is not None:
        return key_error
    selected = [field for field in ("ref", "time", "sequence") if field in value]
    if len(selected) != 1:
        return f"{context}_must_set_exactly_one_cursor"
    field = selected[0]
    if field in {"ref", "time"}:
        return require_non_empty_string(value, field, context)
    return require_positive_int(value, "sequence", context)


def validate_temporal_include(value: Any, context: str) -> str | None:
    if not isinstance(value, dict):
        return f"{context}_not_object"
    key_error = exact_keys(value, {"evidence", "raw_refs", "relations"}, set(), context)
    if key_error is not None:
        return key_error
    for field in ("evidence", "raw_refs", "relations"):
        error = require_bool(value, field, context)
        if error is not None:
            return error
    return None


def validate_inspect_include(value: Any, context: str) -> str | None:
    if not isinstance(value, dict):
        return f"{context}_not_object"
    key_error = exact_keys(
        value, {"details", "incoming", "outgoing", "raw"}, set(), context
    )
    if key_error is not None:
        return key_error
    for field in ("details", "incoming", "outgoing", "raw"):
        error = require_bool(value, field, context)
        if error is not None:
            return error
    if value["raw"]:
        return f"{context}.raw_must_be_false"
    return None


def validate_limit(value: Any, context: str) -> str | None:
    if not isinstance(value, dict):
        return f"{context}_not_object"
    key_error = exact_keys(value, {"entries", "tokens"}, set(), context)
    if key_error is not None:
        return key_error
    for field in ("entries", "tokens"):
        error = require_positive_int(value, field, context)
        if error is not None:
            return error
    return None


def validate_budget(value: Any, context: str) -> str | None:
    if not isinstance(value, dict):
        return f"{context}_not_object"
    key_error = exact_keys(value, set(), {"tokens", "depth", "detail"}, context)
    if key_error is not None:
        return key_error
    if not value:
        return f"{context}_empty"
    for field in ("tokens", "depth"):
        error = validate_optional_positive_int(value, field, context)
        if error is not None:
            return error
    detail = value.get("detail")
    if detail is not None and detail not in {"compact", "balanced", "full"}:
        return f"{context}.detail_unsupported"
    return None


def validate_window(value: Any, context: str) -> str | None:
    if not isinstance(value, dict):
        return f"{context}_not_object"
    key_error = exact_keys(value, {"before_entries", "after_entries"}, set(), context)
    if key_error is not None:
        return key_error
    for field in ("before_entries", "after_entries"):
        error = require_nonnegative_int(value, field, context)
        if error is not None:
            return error
    return None


def validate_page(value: Any, context: str) -> str | None:
    if not isinstance(value, dict):
        return f"{context}_not_object"
    key_error = exact_keys(value, set(), {"entries", "cursor"}, context)
    if key_error is not None:
        return key_error
    error = validate_optional_positive_int(value, "entries", context)
    if error is not None:
        return error
    return validate_optional_non_empty_string(value, "cursor", context)


def validate_answer_policy(value: Any) -> str | None:
    if value not in {"evidence_or_unknown", "show_conflicts", "best_effort"}:
        return "unsupported_answer_policy"
    return None


def is_bounded_tool_call(tool: str, arguments: dict[str, Any]) -> bool:
    if tool == "kernel_wake":
        return (
            path_non_empty_string(arguments, ("about",))
            and optional_limit(arguments, ("budget", "tokens"), 16_000)
            and optional_limit(arguments, ("budget", "depth"), 8)
            and optional_limit(arguments, ("depth",), 8)
        )
    if tool == "kernel_near":
        return (
            positive_limit(arguments, ("limit", "entries"), 64)
            and positive_limit(arguments, ("limit", "tokens"), 16_000)
            and optional_limit(arguments, ("budget", "tokens"), 16_000)
            and optional_limit(arguments, ("budget", "depth"), 8)
            and optional_limit(arguments, ("window", "before_entries"), 64)
            and optional_limit(arguments, ("window", "after_entries"), 64)
            and path_cursor(arguments, ("around",)) is not None
        )
    if tool == "kernel_trace":
        return (
            path_string(arguments, ("from",)) is not None
            and path_string(arguments, ("to",)) is not None
            and positive_limit(arguments, ("budget", "tokens"), 16_000)
            and optional_limit(arguments, ("budget", "depth"), 8)
            and optional_limit(arguments, ("page", "entries"), 256)
        )
    if tool == "kernel_inspect":
        return (
            path_string(arguments, ("ref",)) is not None
            and arguments.get("include", {}).get("raw") is False
        )
    if tool in {"kernel_goto", "kernel_rewind", "kernel_forward"}:
        cursor_key = "at" if tool == "kernel_goto" else "from"
        return (
            path_cursor(arguments, (cursor_key,)) is not None
            and optional_limit(arguments, ("limit", "entries"), 64)
            and optional_limit(arguments, ("limit", "tokens"), 16_000)
            and optional_limit(arguments, ("budget", "tokens"), 16_000)
        )
    if tool == "kernel_ask":
        return optional_limit(arguments, ("budget", "tokens"), 16_000)
    return False


def exact_keys(
    value: dict[str, Any],
    required: set[str],
    optional: set[str],
    context: str,
) -> str | None:
    missing = sorted(required - set(value.keys()))
    if missing:
        return f"{context}_missing_required:{','.join(missing)}"
    unexpected = sorted(set(value.keys()) - required - optional)
    if unexpected:
        return f"{context}_unexpected:{','.join(unexpected)}"
    return None


def require_non_empty_string(value: dict[str, Any], field: str, context: str) -> str | None:
    actual = value.get(field)
    if not isinstance(actual, str) or not actual:
        return f"{context}.{field}_missing_or_empty"
    return None


def validate_optional_non_empty_string(
    value: dict[str, Any], field: str, context: str
) -> str | None:
    if field not in value:
        return None
    return require_non_empty_string(value, field, context)


def require_bool(value: dict[str, Any], field: str, context: str) -> str | None:
    if not isinstance(value.get(field), bool):
        return f"{context}.{field}_not_bool"
    return None


def require_positive_int(value: dict[str, Any], field: str, context: str) -> str | None:
    actual = value.get(field)
    if not isinstance(actual, int) or isinstance(actual, bool) or actual <= 0:
        return f"{context}.{field}_not_positive_int"
    return None


def require_nonnegative_int(value: dict[str, Any], field: str, context: str) -> str | None:
    actual = value.get(field)
    if not isinstance(actual, int) or isinstance(actual, bool) or actual < 0:
        return f"{context}.{field}_not_nonnegative_int"
    return None


def validate_optional_positive_int(
    value: dict[str, Any], field: str, context: str
) -> str | None:
    if field not in value:
        return None
    return require_positive_int(value, field, context)


def validate_string_array(value: Any, context: str) -> str | None:
    if not isinstance(value, list) or not all(
        isinstance(item, str) and item for item in value
    ):
        return f"{context}_not_string_array"
    return None


def positive_limit(value: dict[str, Any], path: tuple[str, ...], maximum: int) -> bool:
    actual = path_int(value, path)
    return actual is not None and 0 < actual <= maximum


def optional_limit(value: dict[str, Any], path: tuple[str, ...], maximum: int) -> bool:
    actual = path_int(value, path)
    return actual is None or actual <= maximum


def path_int(value: dict[str, Any], path: tuple[str, ...]) -> int | None:
    current: Any = value
    for key in path:
        if not isinstance(current, dict):
            return None
        current = current.get(key)
    if isinstance(current, bool) or not isinstance(current, int):
        return None
    return current


def path_string(value: dict[str, Any], path: tuple[str, ...]) -> str | None:
    current: Any = value
    for key in path:
        if not isinstance(current, dict):
            return None
        current = current.get(key)
    return current if isinstance(current, str) and current else None


def path_non_empty_string(value: dict[str, Any], path: tuple[str, ...]) -> bool:
    return path_string(value, path) is not None


def path_cursor(value: dict[str, Any], path: tuple[str, ...]) -> tuple[str, Any] | None:
    current: Any = value
    for key in path:
        if not isinstance(current, dict):
            return None
        current = current.get(key)
    if not isinstance(current, dict):
        return None
    selected = [(key, current[key]) for key in ("ref", "time", "sequence") if key in current]
    if len(selected) != 1:
        return None
    key, actual = selected[0]
    if key in {"ref", "time"}:
        return (key, actual) if isinstance(actual, str) and actual else None
    if isinstance(actual, bool) or not isinstance(actual, int) or actual <= 0:
        return None
    return key, actual


if __name__ == "__main__":
    main()
