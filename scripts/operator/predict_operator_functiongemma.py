#!/usr/bin/env python3
"""Generate policy-eval predictions with FunctionGemma-native tool calls."""

from __future__ import annotations

import argparse
import json
import shutil
import sys
from pathlib import Path
from typing import Any

from functiongemma_operator import (
    KMP_FUNCTION_TOOLS,
    build_prompt_messages,
    function_call_to_action,
    parse_function_call,
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Predict KMP operator actions with FunctionGemma-native calls."
    )
    parser.add_argument("--dataset-jsonl", required=True, type=Path)
    parser.add_argument("--model-id", default="google/functiongemma-270m-it")
    parser.add_argument("--adapter", default=None)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--limit", type=int, default=None)
    parser.add_argument("--batch-size", type=int, default=8)
    parser.add_argument("--max-new-tokens", type=int, default=220)
    parser.add_argument("--temperature", type=float, default=0.0)
    parser.add_argument("--force", action="store_true")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    if args.output.exists():
        if not args.force:
            raise SystemExit(f"output already exists: {args.output}; pass --force")
        shutil.rmtree(args.output)
    args.output.mkdir(parents=True)

    try:
        import torch
        from peft import PeftModel
        from transformers import AutoModelForCausalLM, AutoTokenizer, LogitsProcessorList
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
                    build_prompt_messages(row),
                    tools=KMP_FUNCTION_TOOLS,
                    tokenize=False,
                    add_generation_prompt=True,
                )
                for row in batch
            ]
            inputs = tokenizer(prompts, return_tensors="pt", padding=True).to(model.device)
            prompt_width = inputs["input_ids"].shape[-1]
            generation_kwargs = {
                "max_new_tokens": args.max_new_tokens,
                "do_sample": args.temperature > 0,
                "eos_token_id": tokenizer.eos_token_id,
                "pad_token_id": tokenizer.eos_token_id,
                "logits_processor": LogitsProcessorList(
                    [
                        StopAfterFunctionCallProcessor(
                            tokenizer=tokenizer,
                            prompt_width=prompt_width,
                            eos_token_id=tokenizer.eos_token_id,
                        )
                    ]
                ),
            }
            if args.temperature > 0:
                generation_kwargs["temperature"] = args.temperature
            with torch.inference_mode():
                output = model.generate(**inputs, **generation_kwargs)

            for row, generated_output in zip(batch, output, strict=True):
                generated = generated_output[prompt_width:]
                raw = tokenizer.decode(generated, skip_special_tokens=True).strip()
                action: dict[str, Any] | None = None
                failure_reason: str | None = None
                try:
                    function_name, arguments = parse_function_call(raw)
                    action = function_call_to_action(function_name, arguments)
                except ValueError as exc:
                    failure_reason = str(exc)

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
                    failure_handle.write(
                        json.dumps(
                            {
                                "step_id": row["step_id"],
                                "reason": failure_reason or "invalid_function_call",
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
                        "event": "kernel_operator_functiongemma_predict.progress",
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
        "predictor": "kernel-operator-functiongemma-native-predict-v1",
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


class StopAfterFunctionCallProcessor:
    """Force EOS once each row has emitted a complete FunctionGemma call."""

    def __init__(self, tokenizer: Any, prompt_width: int, eos_token_id: int) -> None:
        self.tokenizer = tokenizer
        self.prompt_width = prompt_width
        self.eos_token_id = eos_token_id

    def __call__(self, input_ids: Any, scores: Any) -> Any:
        for row_index in range(input_ids.shape[0]):
            generated_ids = input_ids[row_index, self.prompt_width :]
            text = self.tokenizer.decode(generated_ids, skip_special_tokens=True)
            if "<end_function_call>" in text:
                scores[row_index].fill_(-float("inf"))
                scores[row_index, self.eos_token_id] = 0
        return scores


def read_jsonl(path: Path) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    with path.open(encoding="utf-8") as handle:
        for line in handle:
            if line.strip():
                rows.append(json.loads(line))
    return rows


if __name__ == "__main__":
    main()
