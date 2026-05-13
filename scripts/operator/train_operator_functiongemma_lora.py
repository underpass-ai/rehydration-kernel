#!/usr/bin/env python3
"""Train FunctionGemma-native LoRA SFT for KMP tool operation."""

from __future__ import annotations

import argparse
import inspect
import json
import os
import shutil
import time
from pathlib import Path
from typing import Any

from functiongemma_operator import (
    KMP_FUNCTION_TOOLS,
    build_prompt_messages,
    format_function_call,
    target_function_call_from_row,
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Train FunctionGemma-native KMP operator LoRA SFT model."
    )
    parser.add_argument("--train-jsonl", required=True, type=Path)
    parser.add_argument("--eval-jsonl", required=True, type=Path)
    parser.add_argument("--model-id", default="google/functiongemma-270m-it")
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument("--epochs", type=float, default=3.0)
    parser.add_argument("--learning-rate", type=float, default=2e-4)
    parser.add_argument("--batch-size", type=int, default=2)
    parser.add_argument("--grad-accum", type=int, default=8)
    parser.add_argument("--max-length", type=int, default=2048)
    parser.add_argument(
        "--device-map",
        choices=["auto", "none"],
        default="auto",
        help="Use 'none' for torchrun/DDP so Accelerate owns device placement.",
    )
    parser.add_argument("--lora-r", type=int, default=16)
    parser.add_argument("--lora-alpha", type=int, default=32)
    parser.add_argument("--bf16", action="store_true")
    parser.add_argument("--fp16", action="store_true")
    parser.add_argument("--force", action="store_true")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    prepare_output_dir(args.output_dir, args.force)

    try:
        from datasets import Dataset
        from peft import LoraConfig
        from transformers import AutoModelForCausalLM, AutoTokenizer
        from trl import SFTConfig, SFTTrainer
    except ImportError as exc:
        raise SystemExit(
            "Missing training dependencies. Install torch, transformers, datasets, "
            "peft, accelerate, and trl in the training environment."
        ) from exc

    tokenizer = AutoTokenizer.from_pretrained(args.model_id, use_fast=True)
    if tokenizer.pad_token is None:
        tokenizer.pad_token = tokenizer.eos_token

    train_rows = build_text_rows(read_jsonl(args.train_jsonl), tokenizer)
    eval_rows = build_text_rows(read_jsonl(args.eval_jsonl), tokenizer)
    if process_rank() == 0:
        write_jsonl(args.output_dir / "functiongemma_train_text.jsonl", train_rows)
        write_jsonl(args.output_dir / "functiongemma_eval_text.jsonl", eval_rows)

    model_kwargs: dict[str, Any] = {
        "torch_dtype": "auto",
    }
    if args.device_map != "none":
        model_kwargs["device_map"] = args.device_map
    model = AutoModelForCausalLM.from_pretrained(args.model_id, **model_kwargs)

    sft_config_kwargs: dict[str, Any] = {
        "output_dir": str(args.output_dir),
        "max_length": args.max_length,
        "num_train_epochs": args.epochs,
        "per_device_train_batch_size": args.batch_size,
        "per_device_eval_batch_size": args.batch_size,
        "gradient_accumulation_steps": args.grad_accum,
        "learning_rate": args.learning_rate,
        "lr_scheduler_type": "cosine",
        "warmup_ratio": 0.03,
        "logging_steps": 10,
        "save_strategy": "epoch",
        "eval_strategy": "epoch" if eval_rows else "no",
        "bf16": args.bf16,
        "fp16": args.fp16,
        "packing": False,
        "ddp_find_unused_parameters": False,
        "report_to": "none",
    }
    if "dataset_text_field" in inspect.signature(SFTConfig).parameters:
        sft_config_kwargs["dataset_text_field"] = "text"

    trainer_kwargs: dict[str, Any] = {
        "model": model,
        "processing_class": tokenizer,
        "train_dataset": Dataset.from_list(train_rows),
        "eval_dataset": Dataset.from_list(eval_rows) if eval_rows else None,
        "peft_config": LoraConfig(
            r=args.lora_r,
            lora_alpha=args.lora_alpha,
            lora_dropout=0.05,
            bias="none",
            task_type="CAUSAL_LM",
            target_modules=[
                "q_proj",
                "k_proj",
                "v_proj",
                "o_proj",
                "gate_proj",
                "up_proj",
                "down_proj",
            ],
        ),
        "args": SFTConfig(**sft_config_kwargs),
    }
    if "dataset_text_field" not in sft_config_kwargs and "formatting_func" in inspect.signature(
        SFTTrainer
    ).parameters:
        trainer_kwargs["formatting_func"] = lambda example: example["text"]

    trainer = SFTTrainer(**trainer_kwargs)
    trainer.train()
    trainer.save_model(str(args.output_dir))
    if trainer.is_world_process_zero():
        tokenizer.save_pretrained(str(args.output_dir))

        summary = {
            "trainer": "kernel-operator-functiongemma-native-lora-v1",
            "model_id": args.model_id,
            "train_rows": len(train_rows),
            "eval_rows": len(eval_rows),
            "tools": [tool["function"]["name"] for tool in KMP_FUNCTION_TOOLS],
        }
        (args.output_dir / "functiongemma_summary.json").write_text(
            json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )


def prepare_output_dir(path: Path, force: bool) -> None:
    ready_path = path / ".functiongemma_output_ready"
    if process_rank() == 0:
        if path.exists():
            if not force:
                raise SystemExit(f"output already exists: {path}; pass --force")
            shutil.rmtree(path)
        path.mkdir(parents=True)
        ready_path.write_text("ready\n", encoding="utf-8")
        return

    deadline = time.monotonic() + 300
    while time.monotonic() < deadline:
        if ready_path.exists():
            return
        time.sleep(0.2)
    raise SystemExit(f"timed out waiting for rank 0 to prepare output dir: {path}")


def process_rank() -> int:
    return int(os.environ.get("RANK", "0"))


def build_text_rows(rows: list[dict[str, Any]], tokenizer: Any) -> list[dict[str, str]]:
    text_rows: list[dict[str, str]] = []
    eos = tokenizer.eos_token or ""
    for row in rows:
        prompt = tokenizer.apply_chat_template(
            build_prompt_messages(row),
            tools=KMP_FUNCTION_TOOLS,
            tokenize=False,
            add_generation_prompt=True,
        )
        function_name, arguments = target_function_call_from_row(row)
        completion = format_function_call(function_name, arguments)
        text_rows.append({"text": prompt + completion + eos})
    return text_rows


def read_jsonl(path: Path) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    with path.open(encoding="utf-8") as handle:
        for line in handle:
            if line.strip():
                rows.append(json.loads(line))
    return rows


def write_jsonl(path: Path, rows: list[dict[str, Any]]) -> None:
    with path.open("w", encoding="utf-8") as handle:
        for row in rows:
            handle.write(json.dumps(row, separators=(",", ":"), sort_keys=True))
            handle.write("\n")


if __name__ == "__main__":
    main()
