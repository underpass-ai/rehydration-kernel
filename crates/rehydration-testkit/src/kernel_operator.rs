use serde_json::Value;

pub fn kernel_operator_allowed_read_tools() -> Vec<String> {
    [
        "kernel_near",
        "kernel_trace",
        "kernel_inspect",
        "kernel_goto",
        "kernel_rewind",
        "kernel_forward",
        "kernel_ask",
    ]
    .iter()
    .map(ToString::to_string)
    .collect()
}

pub fn kernel_operator_is_bounded_tool_call(tool: &str, arguments: &Value) -> bool {
    match tool {
        "kernel_near" => {
            positive_limit(arguments, &["limit", "entries"], 64)
                && positive_limit(arguments, &["limit", "tokens"], 16_000)
                && optional_limit(arguments, &["budget", "tokens"], 16_000)
                && optional_limit(arguments, &["budget", "depth"], 8)
                && optional_limit(arguments, &["window", "before_entries"], 64)
                && optional_limit(arguments, &["window", "after_entries"], 64)
                && path_string(arguments, &["around", "ref"]).is_some()
        }
        "kernel_trace" => {
            path_string(arguments, &["from"]).is_some()
                && path_string(arguments, &["to"]).is_some()
                && positive_limit(arguments, &["budget", "tokens"], 16_000)
                && optional_limit(arguments, &["budget", "depth"], 8)
                && optional_limit(arguments, &["page", "entries"], 256)
        }
        "kernel_inspect" => {
            path_string(arguments, &["ref"]).is_some()
                && arguments
                    .pointer("/include/raw")
                    .and_then(Value::as_bool)
                    .is_some_and(|raw| !raw)
        }
        "kernel_goto" | "kernel_rewind" | "kernel_forward" => {
            optional_limit(arguments, &["limit", "entries"], 64)
                && optional_limit(arguments, &["limit", "tokens"], 16_000)
                && optional_limit(arguments, &["budget", "tokens"], 16_000)
        }
        "kernel_ask" => optional_limit(arguments, &["budget", "tokens"], 16_000),
        _ => false,
    }
}

pub fn kernel_operator_primary_refs(action: &Value) -> Vec<String> {
    let Some(arguments) = action.get("arguments") else {
        return Vec::new();
    };
    let Some(tool) = action.get("tool").and_then(Value::as_str) else {
        return Vec::new();
    };
    match tool {
        "kernel_near" => path_string(arguments, &["around", "ref"])
            .map(|value| vec![value.to_string()])
            .unwrap_or_default(),
        "kernel_inspect" => path_string(arguments, &["ref"])
            .map(|value| vec![value.to_string()])
            .unwrap_or_default(),
        "kernel_trace" => {
            let mut refs = Vec::new();
            if let Some(from) = path_string(arguments, &["from"]) {
                refs.push(from.to_string());
            }
            if let Some(to) = path_string(arguments, &["to"]) {
                refs.push(to.to_string());
            }
            refs
        }
        "kernel_goto" | "kernel_rewind" | "kernel_forward" => path_string(arguments, &["ref"])
            .map(|value| vec![value.to_string()])
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn positive_limit(value: &Value, path: &[&str], max: u64) -> bool {
    path_u64(value, path).is_some_and(|actual| actual > 0 && actual <= max)
}

fn optional_limit(value: &Value, path: &[&str], max: u64) -> bool {
    path_u64(value, path).is_none_or(|actual| actual <= max)
}

fn path_u64(value: &Value, path: &[&str]) -> Option<u64> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_u64()
}

fn path_string<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_str()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{kernel_operator_is_bounded_tool_call, kernel_operator_primary_refs};

    #[test]
    fn bounded_tool_detection_accepts_expected_navigation_calls() {
        assert!(kernel_operator_is_bounded_tool_call(
            "kernel_near",
            &json!({
                "around": { "ref": "node:1" },
                "limit": { "entries": 12, "tokens": 2400 },
                "budget": { "depth": 3, "tokens": 2400 },
                "window": { "before_entries": 6, "after_entries": 0 }
            })
        ));
        assert!(kernel_operator_is_bounded_tool_call(
            "kernel_trace",
            &json!({
                "from": "node:2",
                "to": "node:1",
                "budget": { "depth": 1, "tokens": 1600 }
            })
        ));
        assert!(kernel_operator_is_bounded_tool_call(
            "kernel_inspect",
            &json!({
                "ref": "node:1",
                "include": { "details": true, "incoming": true, "outgoing": true, "raw": false }
            })
        ));
    }

    #[test]
    fn bounded_tool_detection_rejects_unbounded_calls() {
        assert!(!kernel_operator_is_bounded_tool_call(
            "kernel_near",
            &json!({
                "around": { "ref": "node:1" },
                "limit": { "entries": 500, "tokens": 2400 }
            })
        ));
        assert!(!kernel_operator_is_bounded_tool_call(
            "kernel_inspect",
            &json!({
                "ref": "node:1",
                "include": { "raw": true }
            })
        ));
    }

    #[test]
    fn primary_refs_extracts_tool_ref_shape() {
        assert_eq!(
            kernel_operator_primary_refs(&json!({
                "type": "tool_call",
                "tool": "kernel_trace",
                "arguments": {
                    "from": "node:2",
                    "to": "node:1"
                }
            })),
            ["node:2".to_string(), "node:1".to_string()]
        );
    }
}
