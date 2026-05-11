#!/usr/bin/env python3
"""Generate evaluator-compatible predictions from a trained KMP operator."""

from __future__ import annotations

import argparse
import json
import shutil
import sys
from pathlib import Path
from typing import Any


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Predict KMP operator actions.")
    parser.add_argument("--dataset-jsonl", required=True, type=Path)
    parser.add_argument("--model-id", required=True)
    parser.add_argument("--adapter", default=None)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--limit", type=int, default=None)
    parser.add_argument("--batch-size", type=int, default=8)
    parser.add_argument("--max-new-tokens", type=int, default=350)
    parser.add_argument("--temperature", type=float, default=0.0)
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
        from transformers import AutoModelForCausalLM, AutoTokenizer
    except ImportError as exc:
        raise SystemExit(
            "Missing inference dependencies. Install torch, transformers, peft, "
            "and accelerate in the inference environment."
        ) from exc

    tokenizer = AutoTokenizer.from_pretrained(args.model_id, use_fast=True)
    if tokenizer.pad_token is None:
        tokenizer.pad_token = tokenizer.eos_token
    tokenizer.padding_side = "left"
    model = AutoModelForCausalLM.from_pretrained(
        args.model_id,
        torch_dtype="auto",
        device_map="auto",
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
                output = model.generate(
                    **inputs,
                    **generation_kwargs,
                )

            for row, generated_output in zip(batch, output, strict=True):
                generated = generated_output[prompt_width:]
                raw = tokenizer.decode(generated, skip_special_tokens=True).strip()
                action = parse_action(raw)
                result = {
                    "step_id": row["step_id"],
                    "raw_response": raw,
                    "action": action,
                    "valid_action": action is not None,
                }
                result_handle.write(
                    json.dumps(result, separators=(",", ":"), sort_keys=True) + "\n"
                )
                if action is None:
                    failures += 1
                    failure_handle.write(
                        json.dumps(
                            {
                                "step_id": row["step_id"],
                                "reason": "invalid_json",
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
        "dataset": str(args.dataset_jsonl),
        "model_id": args.model_id,
        "adapter": args.adapter,
        "selected": len(rows),
        "predictions": predictions,
        "failures": failures,
        "batch_size": args.batch_size,
        "max_new_tokens": args.max_new_tokens,
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


def parse_action(raw: str) -> dict[str, Any] | None:
    text = raw.strip()
    if text.startswith("```"):
        text = text.strip("`")
        if text.lower().startswith("json"):
            text = text[4:].strip()
    start = text.find("{")
    end = text.rfind("}")
    if start < 0 or end < start:
        return None
    try:
        value = json.loads(text[start : end + 1])
    except json.JSONDecodeError:
        return None
    action = value.get("action", value)
    if not isinstance(action, dict):
        return None
    return action


if __name__ == "__main__":
    main()
