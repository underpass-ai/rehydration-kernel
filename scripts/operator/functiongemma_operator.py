#!/usr/bin/env python3
"""FunctionGemma-native KMP operator formatting and parsing helpers."""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any

from predict_operator_sft import (
    validate_action_shape,
    validate_allowed_tools_for_user_payload,
)


DEVELOPER_PROMPT = (
    "You are a model that can do function calling with the following functions. "
    "Choose exactly one bounded Underpass KMP tool call. Do not explain. "
    "Do not invent refs, scopes, or hidden memory."
)


USER_POLICY = """Choose the next Kernel Memory Protocol action from the visible state.

Rules:
- Use only refs visible in current_ref, trace_target_ref, candidate_refs, candidate_ref_details, known_refs, or last_observed_refs.
- Prefer candidate_ref_details when choosing between writer candidates.
- Every tool call must be bounded.
- For kernel_near and kernel_ask, about must equal the top-level about value exactly.
- Do not use current_ref as about.
- kernel_inspect.include.raw must be false.
- Use kernel_stop only when the visible state already contains sufficient evidence.
"""


KMP_FUNCTION_TOOLS: list[dict[str, Any]] = [
    {
        "type": "function",
        "function": {
            "name": "kernel_ask",
            "description": "Ask KMP for deterministic evidence under the current about scope.",
            "parameters": {
                "type": "object",
                "properties": {
                    "about": {"type": "string", "description": "Current about id."},
                    "answer_policy": {
                        "type": "string",
                        "description": "Answer policy, usually evidence_or_unknown.",
                    },
                    "dimensions": {
                        "type": "object",
                        "description": "Dimension selection.",
                        "properties": {
                            "mode": {"type": "string", "description": "Selection mode."},
                            "scope": {"type": "string", "description": "Scope."},
                        },
                        "required": ["mode", "scope"],
                    },
                    "question": {"type": "string", "description": "Question to resolve."},
                    "budget": {
                        "type": "object",
                        "description": "Bounded retrieval budget.",
                        "properties": {
                            "tokens": {"type": "integer", "description": "Token budget."}
                        },
                        "required": ["tokens"],
                    },
                },
                "required": ["about", "answer_policy", "dimensions", "question"],
            },
        },
    },
    {
        "type": "function",
        "function": {
            "name": "kernel_near",
            "description": "Read bounded temporal/multidimensional context near a ref.",
            "parameters": {
                "type": "object",
                "properties": {
                    "about": {"type": "string", "description": "Current about id."},
                    "around": {
                        "type": "object",
                        "description": "Anchor ref.",
                        "properties": {
                            "ref": {"type": "string", "description": "Anchor ref."}
                        },
                        "required": ["ref"],
                    },
                    "dimensions": {
                        "type": "object",
                        "description": "Dimension selection.",
                        "properties": {
                            "mode": {"type": "string", "description": "Selection mode."},
                            "scope": {"type": "string", "description": "Scope."},
                        },
                        "required": ["mode", "scope"],
                    },
                    "include": {
                        "type": "object",
                        "description": "Included evidence shape.",
                        "properties": {
                            "evidence": {"type": "boolean", "description": "Include evidence."},
                            "raw_refs": {"type": "boolean", "description": "Include raw refs."},
                            "relations": {"type": "boolean", "description": "Include relations."},
                        },
                        "required": ["evidence", "raw_refs", "relations"],
                    },
                    "limit": {
                        "type": "object",
                        "description": "Entry and token limits.",
                        "properties": {
                            "entries": {"type": "integer", "description": "Entry limit."},
                            "tokens": {"type": "integer", "description": "Token limit."},
                        },
                        "required": ["entries", "tokens"],
                    },
                    "budget": {
                        "type": "object",
                        "description": "Traversal budget.",
                        "properties": {
                            "depth": {"type": "integer", "description": "Depth limit."},
                            "tokens": {"type": "integer", "description": "Token budget."},
                        },
                        "required": ["depth", "tokens"],
                    },
                    "window": {
                        "type": "object",
                        "description": "Temporal window.",
                        "properties": {
                            "before_entries": {
                                "type": "integer",
                                "description": "Entries before anchor.",
                            },
                            "after_entries": {
                                "type": "integer",
                                "description": "Entries after anchor.",
                            },
                        },
                        "required": ["before_entries", "after_entries"],
                    },
                },
                "required": ["about", "around", "dimensions", "include", "limit", "budget"],
            },
        },
    },
    {
        "type": "function",
        "function": {
            "name": "kernel_inspect",
            "description": "Inspect one visible KMP ref without raw memory.",
            "parameters": {
                "type": "object",
                "properties": {
                    "ref": {"type": "string", "description": "Visible ref to inspect."},
                    "include": {
                        "type": "object",
                        "description": "Inspect include flags.",
                        "properties": {
                            "details": {"type": "boolean", "description": "Include details."},
                            "incoming": {"type": "boolean", "description": "Incoming relations."},
                            "outgoing": {"type": "boolean", "description": "Outgoing relations."},
                            "raw": {"type": "boolean", "description": "Raw memory, must be false."},
                        },
                        "required": ["details", "incoming", "outgoing", "raw"],
                    },
                },
                "required": ["ref", "include"],
            },
        },
    },
    {
        "type": "function",
        "function": {
            "name": "kernel_trace",
            "description": "Trace a bounded path between two visible refs.",
            "parameters": {
                "type": "object",
                "properties": {
                    "from": {"type": "string", "description": "Start ref."},
                    "to": {"type": "string", "description": "Target ref."},
                    "goal": {"type": "string", "description": "Trace goal."},
                    "budget": {
                        "type": "object",
                        "description": "Trace budget.",
                        "properties": {
                            "depth": {"type": "integer", "description": "Depth limit."},
                            "tokens": {"type": "integer", "description": "Token budget."},
                        },
                        "required": ["depth", "tokens"],
                    },
                },
                "required": ["from", "to", "goal", "budget"],
            },
        },
    },
    {
        "type": "function",
        "function": {
            "name": "kernel_stop",
            "description": "Stop when sufficient bounded evidence is already visible.",
            "parameters": {
                "type": "object",
                "properties": {
                    "answer_policy": {
                        "type": "string",
                        "description": "Answer policy, usually evidence_or_unknown.",
                    },
                    "final_refs": {
                        "type": "array",
                        "description": "Final evidence refs.",
                        "items": {"type": "string"},
                    },
                    "reason": {"type": "string", "description": "Stop reason."},
                },
                "required": ["answer_policy", "final_refs", "reason"],
            },
        },
    },
]


def build_prompt_messages(row: dict[str, Any]) -> list[dict[str, str]]:
    user_content = row["messages"][1]["content"]
    return [
        {"role": "developer", "content": DEVELOPER_PROMPT},
        {"role": "user", "content": f"{USER_POLICY}\nVisible state:\n{user_content}"},
    ]


def target_function_call_from_row(row: dict[str, Any]) -> tuple[str, dict[str, Any]]:
    action = target_action_from_row(row)
    return action_to_function_call(action)


def validate_native_supported_dataset(rows: list[dict[str, Any]]) -> None:
    supported_tools = supported_function_names() - {"kernel_stop"}
    for row in rows:
        step_id = row.get("step_id", "<missing-step-id>")
        user_payload = model_facing_user_payload(row)
        mode = user_payload.get("mode")
        if mode != "read":
            raise ValueError(
                f"{step_id}: FunctionGemma native path is legacy/read-only and "
                f"does not support mode `{mode}`."
            )
        allowed_tools = user_payload.get("allowed_tools")
        if not isinstance(allowed_tools, list) or not all(
            isinstance(tool, str) and tool for tool in allowed_tools
        ):
            raise ValueError(f"{step_id}: allowed_tools must be non-empty strings.")
        if len(set(allowed_tools)) != len(allowed_tools):
            raise ValueError(f"{step_id}: duplicate allowed_tools.")
        allowed_mode_error = validate_allowed_tools_for_user_payload(user_payload)
        if allowed_mode_error is not None:
            raise ValueError(f"{step_id}: {allowed_mode_error}.")
        unsupported_allowed = sorted(
            {
                tool
                for tool in allowed_tools
                if tool not in supported_tools and tool != "kernel_stop"
            }
        )
        if unsupported_allowed:
            raise ValueError(
                f"{step_id}: FunctionGemma native path supports only "
                f"{sorted(supported_tools)} in allowed_tools; got "
                f"{unsupported_allowed}."
            )

        action = target_action_from_row(row)
        shape_error = validate_action_shape(action)
        if shape_error is not None:
            raise ValueError(
                f"{step_id}: FunctionGemma native target violates strict "
                f"action contract: {shape_error}."
            )
        action_type = action.get("type")
        if action_type == "prepared_tool_call":
            raise ValueError(
                f"{step_id}: FunctionGemma native path does not support "
                "prepared_tool_call; use predict_operator_sft.py "
                "--resolve-prepared-payloads for writer-exec/orchestration."
            )
        if action_type == "tool_call":
            tool = action.get("tool")
            if tool not in allowed_tools:
                raise ValueError(
                    f"{step_id}: target tool `{tool}` is not listed in row allowed_tools."
                )
            if tool not in supported_tools:
                raise ValueError(
                    f"{step_id}: FunctionGemma native path supports only "
                    f"{sorted(supported_tools)} target tools; got `{tool}`."
                )
            continue
        if action_type == "stop":
            continue
        raise ValueError(
            f"{step_id}: FunctionGemma native path does not support target "
            f"action type `{action_type}`."
        )


def target_action_from_row(row: dict[str, Any]) -> dict[str, Any]:
    messages = row.get("messages")
    if not isinstance(messages, list) or not messages:
        raise ValueError(f"{row.get('step_id', '<missing-step-id>')}: missing messages")
    assistant = messages[-1]
    if not isinstance(assistant, dict) or not isinstance(assistant.get("content"), str):
        raise ValueError(
            f"{row.get('step_id', '<missing-step-id>')}: missing assistant target"
        )
    try:
        payload = json.loads(assistant["content"])
    except json.JSONDecodeError as exc:
        raise ValueError(
            f"{row.get('step_id', '<missing-step-id>')}: invalid assistant JSON"
        ) from exc
    action = payload.get("action")
    if not isinstance(action, dict):
        raise ValueError(f"{row.get('step_id', '<missing-step-id>')}: missing action")
    return action


def model_facing_user_payload(row: dict[str, Any]) -> dict[str, Any]:
    messages = row.get("messages")
    if not isinstance(messages, list) or len(messages) < 2:
        raise ValueError(f"{row.get('step_id', '<missing-step-id>')}: missing user payload")
    user = messages[1]
    if not isinstance(user, dict) or not isinstance(user.get("content"), str):
        raise ValueError(
            f"{row.get('step_id', '<missing-step-id>')}: missing user content"
        )
    try:
        payload = json.loads(user["content"])
    except json.JSONDecodeError as exc:
        raise ValueError(
            f"{row.get('step_id', '<missing-step-id>')}: invalid user JSON"
        ) from exc
    if not isinstance(payload, dict):
        raise ValueError(
            f"{row.get('step_id', '<missing-step-id>')}: user payload is not an object"
        )
    return payload


def action_to_function_call(action: dict[str, Any]) -> tuple[str, dict[str, Any]]:
    action_type = action.get("type")
    if action_type == "tool_call":
        tool = action.get("tool")
        arguments = action.get("arguments")
        if not isinstance(tool, str) or not isinstance(arguments, dict):
            raise ValueError(f"invalid tool_call action: {action}")
        if tool not in supported_function_names() or tool == "kernel_stop":
            raise ValueError(f"unsupported action tool for FunctionGemma: {tool}")
        return tool, arguments
    if action_type == "stop":
        return "kernel_stop", {
            "answer_policy": action.get("answer_policy", "evidence_or_unknown"),
            "final_refs": action.get("final_refs", []),
            "reason": action.get("reason", "sufficient_evidence"),
        }
    raise ValueError(f"unsupported action type for FunctionGemma: {action_type}")


def function_call_to_action(name: str, arguments: dict[str, Any]) -> dict[str, Any]:
    if name == "kernel_stop":
        return {
            "type": "stop",
            "answer_policy": arguments.get("answer_policy", "evidence_or_unknown"),
            "final_refs": arguments.get("final_refs", []),
            "reason": arguments.get("reason", "sufficient_evidence"),
        }
    if name not in supported_function_names():
        raise ValueError(f"unsupported function name: {name}")
    return {"type": "tool_call", "tool": name, "arguments": arguments}


def supported_function_names() -> set[str]:
    return {tool["function"]["name"] for tool in KMP_FUNCTION_TOOLS}


def format_function_call(name: str, arguments: dict[str, Any]) -> str:
    return (
        f"<start_function_call>call:{name}"
        f"{{{format_object_items(arguments)}}}<end_function_call>"
    )


def format_object_items(value: dict[str, Any]) -> str:
    return ",".join(f"{key}:{format_argument(child)}" for key, child in sorted(value.items()))


def format_argument(value: Any) -> str:
    if isinstance(value, str):
        if "<escape>" in value:
            raise ValueError("FunctionGemma argument strings cannot contain <escape>")
        return f"<escape>{value}<escape>"
    if isinstance(value, bool):
        return "true" if value else "false"
    if value is None:
        return "null"
    if isinstance(value, (int, float)):
        return str(value)
    if isinstance(value, list):
        return "[" + ",".join(format_argument(item) for item in value) + "]"
    if isinstance(value, dict):
        return "{" + format_object_items(value) + "}"
    raise TypeError(f"unsupported FunctionGemma argument type: {type(value).__name__}")


def parse_function_call(raw: str) -> tuple[str, dict[str, Any]]:
    text = raw.strip()
    start_marker = "<start_function_call>call:"
    end_marker = "<end_function_call>"
    if not text.startswith(start_marker):
        raise ValueError("missing_function_call_start")
    end = text.find(end_marker)
    if end < 0:
        raise ValueError("incomplete_function_call")
    suffix = text[end + len(end_marker) :].strip()
    if suffix:
        raise ValueError("extra_content_after_function_call")
    body = text[len(start_marker) : end]
    brace = body.find("{")
    if brace < 1 or not body.endswith("}"):
        raise ValueError("invalid_function_call_shape")
    name = body[:brace]
    parser = _FunctionGemmaArgumentParser(body[brace:])
    arguments = parser.parse_object()
    parser.expect_end()
    return name, arguments


@dataclass
class _FunctionGemmaArgumentParser:
    text: str
    index: int = 0

    def parse_object(self) -> dict[str, Any]:
        self._consume("{")
        result: dict[str, Any] = {}
        self._skip_ws()
        if self._peek() == "}":
            self.index += 1
            return result
        while True:
            key = self._parse_key()
            self._consume(":")
            result[key] = self.parse_value()
            self._skip_ws()
            char = self._peek()
            if char == ",":
                self.index += 1
                continue
            if char == "}":
                self.index += 1
                return result
            raise ValueError("invalid_object_separator")

    def parse_value(self) -> Any:
        self._skip_ws()
        if self.text.startswith("<escape>", self.index):
            return self._parse_escaped_string()
        char = self._peek()
        if char == "{":
            return self.parse_object()
        if char == "[":
            return self._parse_array()
        token = self._parse_bare_token()
        if token == "true":
            return True
        if token == "false":
            return False
        if token == "null":
            return None
        try:
            return int(token)
        except ValueError:
            try:
                return float(token)
            except ValueError:
                return token

    def expect_end(self) -> None:
        self._skip_ws()
        if self.index != len(self.text):
            raise ValueError("trailing_argument_content")

    def _parse_array(self) -> list[Any]:
        self._consume("[")
        result: list[Any] = []
        self._skip_ws()
        if self._peek() == "]":
            self.index += 1
            return result
        while True:
            result.append(self.parse_value())
            self._skip_ws()
            char = self._peek()
            if char == ",":
                self.index += 1
                continue
            if char == "]":
                self.index += 1
                return result
            raise ValueError("invalid_array_separator")

    def _parse_key(self) -> str:
        self._skip_ws()
        start = self.index
        while self.index < len(self.text) and self.text[self.index] not in ": \t\r\n":
            self.index += 1
        key = self.text[start : self.index]
        if not key:
            raise ValueError("missing_key")
        self._skip_ws()
        return key

    def _parse_escaped_string(self) -> str:
        start_marker = "<escape>"
        end_marker = "<escape>"
        self.index += len(start_marker)
        end = self.text.find(end_marker, self.index)
        if end < 0:
            raise ValueError("unterminated_escaped_string")
        value = self.text[self.index : end]
        self.index = end + len(end_marker)
        return value

    def _parse_bare_token(self) -> str:
        start = self.index
        while self.index < len(self.text) and self.text[self.index] not in ",}] \t\r\n":
            self.index += 1
        token = self.text[start : self.index]
        if not token:
            raise ValueError("missing_value")
        return token

    def _consume(self, expected: str) -> None:
        self._skip_ws()
        if not self.text.startswith(expected, self.index):
            raise ValueError(f"expected_{expected}")
        self.index += len(expected)

    def _peek(self) -> str:
        self._skip_ws()
        if self.index >= len(self.text):
            raise ValueError("unexpected_end")
        return self.text[self.index]

    def _skip_ws(self) -> None:
        while self.index < len(self.text) and self.text[self.index].isspace():
            self.index += 1
