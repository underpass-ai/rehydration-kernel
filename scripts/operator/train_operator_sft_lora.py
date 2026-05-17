#!/usr/bin/env python3
"""Train a small LoRA SFT model for KMP tool operation.

This script is intentionally external to kernel core. It trains a client-side
operator over exported KMP trajectories and produces an adapter that can later
be evaluated offline with `underpass_operator_policy_eval`.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any

from predict_operator_sft import (
    resolve_prepared_payload_action,
    validate_action_shape,
    validate_allowed_tools_for_user_payload,
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Train KMP operator LoRA SFT model.")
    parser.add_argument("--train-jsonl", required=True, type=Path)
    parser.add_argument("--eval-jsonl", required=True, type=Path)
    parser.add_argument("--model-id", default="Qwen/Qwen2.5-0.5B-Instruct")
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument("--hub-model-id", default=None)
    parser.add_argument("--trust-remote-code", action="store_true")
    parser.add_argument(
        "--device-map",
        choices=["auto", "none"],
        default="auto",
        help="Use 'none' for torchrun/DDP so Accelerate owns device placement.",
    )
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
    parser.add_argument("--epochs", type=float, default=3.0)
    parser.add_argument("--learning-rate", type=float, default=2e-4)
    parser.add_argument("--batch-size", type=int, default=2)
    parser.add_argument("--grad-accum", type=int, default=8)
    parser.add_argument("--max-length", type=int, default=2048)
    parser.add_argument("--lora-r", type=int, default=16)
    parser.add_argument("--lora-alpha", type=int, default=32)
    parser.add_argument(
        "--lora-target-modules",
        default="q_proj,k_proj,v_proj,o_proj,gate_proj,up_proj,down_proj",
        help="Comma-separated target modules, or PEFT special value such as all-linear.",
    )
    parser.add_argument("--bf16", action="store_true")
    parser.add_argument("--fp16", action="store_true")
    parser.add_argument("--push-to-hub", action="store_true")
    parser.add_argument(
        "--validate-only",
        action="store_true",
        help="Validate SFT JSONL contract and exit before importing training dependencies.",
    )
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    train_rows = read_jsonl(args.train_jsonl)
    eval_rows = read_jsonl(args.eval_jsonl)
    validate_sft_rows(train_rows, "train")
    validate_sft_rows(eval_rows, "eval")
    validate_no_model_row_overlap(train_rows, eval_rows)
    if args.validate_only:
        print(
            json.dumps(
                {
                    "event": "kernel_operator_sft_train.validate_only",
                    "train_rows": len(train_rows),
                    "eval_rows": len(eval_rows),
                    "status": "ok",
                },
                indent=2,
                sort_keys=True,
            )
        )
        return

    try:
        from datasets import load_dataset
        from peft import LoraConfig
        from transformers import AutoConfig, AutoModelForCausalLM, AutoTokenizer
        from trl import SFTConfig, SFTTrainer
        import torch
    except ImportError as exc:
        raise SystemExit(
            "Missing training dependencies. Install torch, transformers, datasets, "
            "peft, accelerate, and trl in the training environment."
        ) from exc

    dataset = load_dataset(
        "json",
        data_files={
            "train": str(args.train_jsonl),
            "eval": str(args.eval_jsonl),
        },
    )

    tokenizer = AutoTokenizer.from_pretrained(
        args.model_id,
        use_fast=True,
        trust_remote_code=args.trust_remote_code,
    )
    if tokenizer.pad_token is None:
        tokenizer.pad_token = tokenizer.eos_token

    config = AutoConfig.from_pretrained(
        args.model_id,
        trust_remote_code=args.trust_remote_code,
    )
    for key, value in parse_config_overrides(args.config_override).items():
        setattr(config, key, value)

    model_kwargs: dict[str, Any] = {
        "config": config,
        "torch_dtype": torch_dtype(args.torch_dtype, torch),
        "trust_remote_code": args.trust_remote_code,
    }
    if args.device_map != "none":
        model_kwargs["device_map"] = args.device_map
    model = AutoModelForCausalLM.from_pretrained(args.model_id, **model_kwargs)

    lora_target_modules = parse_lora_target_modules(args.lora_target_modules)
    trainer = SFTTrainer(
        model=model,
        processing_class=tokenizer,
        train_dataset=dataset["train"],
        eval_dataset=dataset["eval"] if len(dataset["eval"]) else None,
        peft_config=LoraConfig(
            r=args.lora_r,
            lora_alpha=args.lora_alpha,
            lora_dropout=0.05,
            bias="none",
            task_type="CAUSAL_LM",
            target_modules=lora_target_modules,
        ),
        args=SFTConfig(
            output_dir=str(args.output_dir),
            max_length=args.max_length,
            num_train_epochs=args.epochs,
            per_device_train_batch_size=args.batch_size,
            per_device_eval_batch_size=args.batch_size,
            gradient_accumulation_steps=args.grad_accum,
            learning_rate=args.learning_rate,
            lr_scheduler_type="cosine",
            warmup_ratio=0.03,
            logging_steps=10,
            save_strategy="epoch",
            eval_strategy="epoch" if len(dataset["eval"]) else "no",
            bf16=args.bf16,
            fp16=args.fp16,
            packing=False,
            ddp_find_unused_parameters=False,
            report_to="none",
            push_to_hub=args.push_to_hub,
            hub_model_id=args.hub_model_id,
        ),
    )
    trainer.train()
    trainer.save_model(str(args.output_dir))
    if trainer.is_world_process_zero():
        tokenizer.save_pretrained(str(args.output_dir))
    if args.push_to_hub and trainer.is_world_process_zero():
        trainer.push_to_hub()


def parse_lora_target_modules(value: str) -> str | list[str]:
    if value == "all-linear":
        return value
    modules = [part.strip() for part in value.split(",") if part.strip()]
    if not modules:
        raise SystemExit("--lora-target-modules must not be empty")
    return modules


def read_jsonl(path: Path) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    with path.open(encoding="utf-8") as handle:
        for line_number, line in enumerate(handle, start=1):
            if not line.strip():
                continue
            try:
                row = json.loads(line)
            except json.JSONDecodeError as exc:
                raise SystemExit(f"{path}:{line_number}: invalid JSONL row") from exc
            if not isinstance(row, dict):
                raise SystemExit(f"{path}:{line_number}: row must be an object")
            rows.append(row)
    return rows


def validate_sft_rows(rows: list[dict[str, Any]], label: str) -> None:
    if label == "train" and not rows:
        raise SystemExit("train dataset must not be empty")

    seen_ids: dict[str, int] = {}
    seen_message_hashes: dict[str, int] = {}
    for index, row in enumerate(rows, start=1):
        row_id = row.get("id") or row.get("step_id") or f"{label}:{index}"
        if isinstance(row_id, str):
            previous = seen_ids.get(row_id)
            if previous is not None:
                raise SystemExit(
                    f"{label} row {index}: duplicate row id `{row_id}` "
                    f"also seen at row {previous}"
                )
            seen_ids[row_id] = index

        messages = row.get("messages")
        if not isinstance(messages, list) or len(messages) != 3:
            raise SystemExit(f"{label} row {index}: expected exactly 3 messages")
        roles = [message.get("role") for message in messages if isinstance(message, dict)]
        if roles != ["system", "user", "assistant"]:
            raise SystemExit(
                f"{label} row {index}: expected system/user/assistant roles, got {roles}"
            )
        if not all(isinstance(message.get("content"), str) for message in messages):
            raise SystemExit(f"{label} row {index}: every message needs string content")

        message_hash = canonical_messages_hash(messages)
        previous_hash = seen_message_hashes.get(message_hash)
        if previous_hash is not None:
            raise SystemExit(
                f"{label} row {index}: duplicate model-facing messages also "
                f"seen at row {previous_hash}"
            )
        seen_message_hashes[message_hash] = index

        user_payload = parse_message_json(messages[1]["content"], label, index, "user")
        allowed_tools = user_payload.get("allowed_tools")
        if not isinstance(allowed_tools, list) or not all(
            isinstance(tool, str) and tool for tool in allowed_tools
        ):
            raise SystemExit(f"{label} row {index}: user allowed_tools must be strings")
        if len(set(allowed_tools)) != len(allowed_tools):
            raise SystemExit(f"{label} row {index}: duplicate allowed_tools")
        allowed_mode_error = validate_allowed_tools_for_user_payload(user_payload)
        if allowed_mode_error is not None:
            raise SystemExit(f"{label} row {index}: {allowed_mode_error}")

        assistant_payload = parse_message_json(
            messages[2]["content"], label, index, "assistant"
        )
        action = assistant_payload.get("action")
        if not isinstance(action, dict):
            raise SystemExit(f"{label} row {index}: assistant payload missing action")
        shape_error = validate_action_shape(action)
        if shape_error is not None:
            raise SystemExit(
                f"{label} row {index}: target action violates strict contract: "
                f"{shape_error}"
            )
        action_type = action.get("type")
        if action_type in {"tool_call", "prepared_tool_call"}:
            tool = action.get("tool")
            if tool not in allowed_tools:
                raise SystemExit(
                    f"{label} row {index}: target tool `{tool}` is not listed in "
                    "row allowed_tools"
                )
        resolved, resolve_error = resolve_prepared_payload_action(action, row)
        if resolve_error is not None:
            raise SystemExit(
                f"{label} row {index}: prepared payload cannot be resolved: "
                f"{resolve_error}"
            )
        if resolved is not None:
            resolved_error = validate_action_shape(resolved)
            if resolved_error is not None:
                raise SystemExit(
                    f"{label} row {index}: resolved target violates strict "
                    f"contract: {resolved_error}"
                )


def parse_message_json(content: str, label: str, index: int, role: str) -> dict[str, Any]:
    try:
        payload = json.loads(content)
    except json.JSONDecodeError as exc:
        raise SystemExit(f"{label} row {index}: invalid {role} JSON content") from exc
    if not isinstance(payload, dict):
        raise SystemExit(f"{label} row {index}: {role} content must be a JSON object")
    return payload


def validate_no_model_row_overlap(
    train_rows: list[dict[str, Any]], eval_rows: list[dict[str, Any]]
) -> None:
    train_hashes = {
        canonical_messages_hash(row["messages"]): index
        for index, row in enumerate(train_rows, start=1)
    }
    for index, row in enumerate(eval_rows, start=1):
        message_hash = canonical_messages_hash(row["messages"])
        train_index = train_hashes.get(message_hash)
        if train_index is not None:
            raise SystemExit(
                "train/eval model-facing overlap: "
                f"eval row {index} duplicates train row {train_index}"
            )


def canonical_messages_hash(messages: list[Any]) -> str:
    payload = json.dumps(messages, separators=(",", ":"), sort_keys=True)
    return hashlib.sha256(payload.encode("utf-8")).hexdigest()


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


if __name__ == "__main__":
    main()
