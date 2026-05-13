#!/usr/bin/env python3
"""Train a small LoRA SFT model for KMP tool operation.

This script is intentionally external to kernel core. It trains a client-side
operator over exported KMP trajectories and produces an adapter that can later
be evaluated offline with `kernel_operator_policy_eval`.
"""

from __future__ import annotations

import argparse
from pathlib import Path
from typing import Any


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
    return parser.parse_args()


def main() -> None:
    args = parse_args()
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
