use serde_json::{Map, Value};

pub fn kernel_operator_allowed_read_tools() -> Vec<String> {
    [
        "kernel_wake",
        "kernel_ask",
        "kernel_near",
        "kernel_goto",
        "kernel_rewind",
        "kernel_forward",
        "kernel_trace",
        "kernel_inspect",
    ]
    .iter()
    .map(ToString::to_string)
    .collect()
}

pub fn kernel_operator_is_bounded_tool_call(tool: &str, arguments: &Value) -> bool {
    match tool {
        "kernel_wake" => {
            path_non_empty_string(arguments, &["about"])
                && optional_limit(arguments, &["budget", "tokens"], 16_000)
                && optional_limit(arguments, &["budget", "depth"], 8)
                && optional_limit(arguments, &["depth"], 8)
        }
        "kernel_near" => {
            positive_limit(arguments, &["limit", "entries"], 64)
                && positive_limit(arguments, &["limit", "tokens"], 16_000)
                && optional_limit(arguments, &["budget", "tokens"], 16_000)
                && optional_limit(arguments, &["budget", "depth"], 8)
                && optional_limit(arguments, &["window", "before_entries"], 64)
                && optional_limit(arguments, &["window", "after_entries"], 64)
                && path_cursor(arguments, &["around"]).is_some()
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
        "kernel_goto" => {
            path_cursor(arguments, &["at"]).is_some()
                && optional_limit(arguments, &["limit", "entries"], 64)
                && optional_limit(arguments, &["limit", "tokens"], 16_000)
                && optional_limit(arguments, &["budget", "tokens"], 16_000)
        }
        "kernel_rewind" | "kernel_forward" => {
            path_cursor(arguments, &["from"]).is_some()
                && optional_limit(arguments, &["limit", "entries"], 64)
                && optional_limit(arguments, &["limit", "tokens"], 16_000)
                && optional_limit(arguments, &["budget", "tokens"], 16_000)
        }
        "kernel_ask" => optional_limit(arguments, &["budget", "tokens"], 16_000),
        _ => false,
    }
}

pub fn kernel_operator_action_shape_error(action: &Value) -> Option<String> {
    validate_action_shape(action).err()
}

pub fn kernel_operator_is_valid_action_shape(action: &Value) -> bool {
    kernel_operator_action_shape_error(action).is_none()
}

pub fn kernel_operator_action_contract_error(action: &Value) -> Option<String> {
    if let Some(error) = kernel_operator_action_shape_error(action) {
        return Some(error);
    }
    let (tool, arguments) = action_tool_arguments(action)?;
    if kernel_operator_is_bounded_tool_call(tool, arguments) {
        None
    } else {
        Some(format!("unbounded or invalid tool call for `{tool}`"))
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
        "kernel_goto" => path_string(arguments, &["at", "ref"])
            .map(|value| vec![value.to_string()])
            .unwrap_or_default(),
        "kernel_rewind" | "kernel_forward" => path_string(arguments, &["from", "ref"])
            .map(|value| vec![value.to_string()])
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn validate_action_shape(action: &Value) -> Result<(), String> {
    object(action, "action")?;
    let action_type = required_string(action, "type", "action")?;
    match action_type {
        "tool_call" => validate_tool_call_shape(action),
        "stop" => validate_stop_shape(action),
        other => Err(format!("unsupported action type `{other}`")),
    }
}

fn validate_tool_call_shape(action: &Value) -> Result<(), String> {
    exact_keys(action, "action", &["type", "tool", "arguments"], &[])?;
    let tool = required_string(action, "tool", "action")?;
    let arguments = required_value(action, "arguments", "action")?;
    object(arguments, "action.arguments")?;
    match tool {
        "kernel_wake" => validate_wake_arguments(arguments),
        "kernel_ask" => validate_ask_arguments(arguments),
        "kernel_near" => validate_temporal_arguments(arguments, "around", "kernel_near"),
        "kernel_goto" => validate_temporal_arguments(arguments, "at", "kernel_goto"),
        "kernel_rewind" => validate_temporal_arguments(arguments, "from", "kernel_rewind"),
        "kernel_forward" => validate_temporal_arguments(arguments, "from", "kernel_forward"),
        "kernel_trace" => validate_trace_arguments(arguments),
        "kernel_inspect" => validate_inspect_arguments(arguments),
        other => Err(format!("unsupported tool `{other}`")),
    }
}

fn validate_stop_shape(action: &Value) -> Result<(), String> {
    exact_keys(
        action,
        "action",
        &["type", "answer_policy", "final_refs", "reason"],
        &[],
    )?;
    validate_answer_policy(required_string(action, "answer_policy", "action")?)?;
    validate_string_array(
        required_value(action, "final_refs", "action")?,
        "action.final_refs",
    )?;
    required_non_empty_string(action, "reason", "action")?;
    Ok(())
}

fn validate_wake_arguments(arguments: &Value) -> Result<(), String> {
    exact_keys(
        arguments,
        "action.arguments",
        &["about"],
        &["role", "intent", "dimensions", "depth", "budget"],
    )?;
    required_non_empty_string(arguments, "about", "action.arguments")?;
    validate_optional_non_empty_string(arguments, "role", "action.arguments")?;
    validate_optional_non_empty_string(arguments, "intent", "action.arguments")?;
    if let Some(dimensions) = arguments.get("dimensions") {
        validate_dimensions(dimensions, "action.arguments.dimensions")?;
    }
    validate_optional_positive_integer(arguments, "depth", "action.arguments")?;
    if let Some(budget) = arguments.get("budget") {
        validate_budget(budget, "action.arguments.budget")?;
    }
    Ok(())
}

fn validate_ask_arguments(arguments: &Value) -> Result<(), String> {
    exact_keys(
        arguments,
        "action.arguments",
        &["about", "answer_policy", "dimensions", "question"],
        &["budget", "depth"],
    )?;
    required_non_empty_string(arguments, "about", "action.arguments")?;
    validate_answer_policy(required_string(
        arguments,
        "answer_policy",
        "action.arguments",
    )?)?;
    validate_dimensions(
        required_value(arguments, "dimensions", "action.arguments")?,
        "action.arguments.dimensions",
    )?;
    required_non_empty_string(arguments, "question", "action.arguments")?;
    if let Some(budget) = arguments.get("budget") {
        validate_budget(budget, "action.arguments.budget")?;
    }
    validate_optional_positive_integer(arguments, "depth", "action.arguments")?;
    Ok(())
}

fn validate_temporal_arguments(
    arguments: &Value,
    cursor_key: &str,
    tool: &str,
) -> Result<(), String> {
    exact_keys(
        arguments,
        "action.arguments",
        &[
            "about",
            cursor_key,
            "dimensions",
            "include",
            "limit",
            "budget",
            "window",
        ],
        &["depth"],
    )?;
    required_non_empty_string(arguments, "about", "action.arguments")?;
    validate_temporal_cursor(
        required_value(arguments, cursor_key, "action.arguments")?,
        &format!("action.arguments.{cursor_key}"),
    )?;
    validate_dimensions(
        required_value(arguments, "dimensions", "action.arguments")?,
        "action.arguments.dimensions",
    )?;
    validate_temporal_include(
        required_value(arguments, "include", "action.arguments")?,
        "action.arguments.include",
    )?;
    validate_limit(
        required_value(arguments, "limit", "action.arguments")?,
        "action.arguments.limit",
    )?;
    validate_budget(
        required_value(arguments, "budget", "action.arguments")?,
        "action.arguments.budget",
    )?;
    validate_window(
        required_value(arguments, "window", "action.arguments")?,
        "action.arguments.window",
    )?;
    validate_optional_positive_integer(arguments, "depth", "action.arguments")?;
    if !kernel_operator_allowed_read_tools()
        .iter()
        .any(|allowed| allowed == tool)
    {
        return Err(format!("unsupported tool `{tool}`"));
    }
    Ok(())
}

fn validate_trace_arguments(arguments: &Value) -> Result<(), String> {
    exact_keys(
        arguments,
        "action.arguments",
        &["from", "to", "budget"],
        &["goal", "role", "page"],
    )?;
    required_non_empty_string(arguments, "from", "action.arguments")?;
    required_non_empty_string(arguments, "to", "action.arguments")?;
    validate_budget(
        required_value(arguments, "budget", "action.arguments")?,
        "action.arguments.budget",
    )?;
    validate_optional_non_empty_string(arguments, "goal", "action.arguments")?;
    validate_optional_non_empty_string(arguments, "role", "action.arguments")?;
    if let Some(page) = arguments.get("page") {
        validate_page(page, "action.arguments.page")?;
    }
    Ok(())
}

fn validate_inspect_arguments(arguments: &Value) -> Result<(), String> {
    exact_keys(arguments, "action.arguments", &["ref", "include"], &[])?;
    required_non_empty_string(arguments, "ref", "action.arguments")?;
    validate_inspect_include(
        required_value(arguments, "include", "action.arguments")?,
        "action.arguments.include",
    )?;
    Ok(())
}

fn validate_dimensions(value: &Value, context: &str) -> Result<(), String> {
    exact_keys(
        value,
        context,
        &["mode", "scope"],
        &["include", "exclude", "scope_ids", "abouts"],
    )?;
    let mode = required_string(value, "mode", context)?;
    if !["all", "only", "except"].contains(&mode) {
        return Err(format!("{context}.mode has unsupported value `{mode}`"));
    }
    let scope = required_string(value, "scope", context)?;
    if !["current_about", "abouts", "all_abouts"].contains(&scope) {
        return Err(format!("{context}.scope has unsupported value `{scope}`"));
    }
    for field in ["include", "exclude", "scope_ids", "abouts"] {
        if let Some(values) = value.get(field) {
            validate_string_array(values, &format!("{context}.{field}"))?;
        }
    }
    let include_count = array_len(value.get("include"));
    let exclude_count = array_len(value.get("exclude"));
    let abouts_count = array_len(value.get("abouts"));
    match mode {
        "all" if include_count > 0 || exclude_count > 0 => {
            return Err(format!(
                "{context}.mode all must not set include or exclude values"
            ));
        }
        "only" if include_count == 0 => {
            return Err(format!("{context}.mode only requires include values"));
        }
        "only" if exclude_count > 0 => {
            return Err(format!("{context}.mode only must not set exclude values"));
        }
        "except" if exclude_count == 0 => {
            return Err(format!("{context}.mode except requires exclude values"));
        }
        "except" if include_count > 0 => {
            return Err(format!("{context}.mode except must not set include values"));
        }
        _ => {}
    }
    match scope {
        "current_about" if abouts_count > 0 => {
            return Err(format!("{context}.scope current_about must not set abouts"));
        }
        "abouts" if abouts_count == 0 => {
            return Err(format!(
                "{context}.scope abouts requires at least one about"
            ));
        }
        "all_abouts" if abouts_count > 0 => {
            return Err(format!("{context}.scope all_abouts must not set abouts"));
        }
        _ => {}
    }
    Ok(())
}

fn validate_temporal_cursor(value: &Value, context: &str) -> Result<(), String> {
    exact_keys(value, context, &[], &["ref", "time", "sequence"])?;
    let object = object(value, context)?;
    let selected = ["ref", "time", "sequence"]
        .iter()
        .filter(|field| object.contains_key(**field))
        .count();
    if selected != 1 {
        return Err(format!(
            "{context} must set exactly one of ref, time, or sequence"
        ));
    }
    if value.get("ref").is_some() {
        required_non_empty_string(value, "ref", context)?;
    }
    if value.get("time").is_some() {
        required_non_empty_string(value, "time", context)?;
    }
    if value.get("sequence").is_some() {
        required_positive_integer(value, "sequence", context)?;
    }
    Ok(())
}

fn validate_temporal_include(value: &Value, context: &str) -> Result<(), String> {
    exact_keys(value, context, &["evidence", "raw_refs", "relations"], &[])?;
    required_bool(value, "evidence", context)?;
    required_bool(value, "raw_refs", context)?;
    required_bool(value, "relations", context)?;
    Ok(())
}

fn validate_inspect_include(value: &Value, context: &str) -> Result<(), String> {
    exact_keys(
        value,
        context,
        &["details", "incoming", "outgoing", "raw"],
        &[],
    )?;
    required_bool(value, "details", context)?;
    required_bool(value, "incoming", context)?;
    required_bool(value, "outgoing", context)?;
    let raw = required_bool(value, "raw", context)?;
    if raw {
        return Err(format!("{context}.raw must be false"));
    }
    Ok(())
}

fn validate_limit(value: &Value, context: &str) -> Result<(), String> {
    exact_keys(value, context, &["entries", "tokens"], &[])?;
    required_positive_integer(value, "entries", context)?;
    required_positive_integer(value, "tokens", context)?;
    Ok(())
}

fn validate_budget(value: &Value, context: &str) -> Result<(), String> {
    exact_keys(value, context, &[], &["tokens", "depth", "detail"])?;
    let object = object(value, context)?;
    if object.is_empty() {
        return Err(format!("{context} must not be empty"));
    }
    validate_optional_positive_integer(value, "tokens", context)?;
    validate_optional_positive_integer(value, "depth", context)?;
    if let Some(detail) = value.get("detail") {
        let Some(detail) = detail.as_str() else {
            return Err(format!("{context}.detail must be a string"));
        };
        if !["compact", "balanced", "full"].contains(&detail) {
            return Err(format!("{context}.detail has unsupported value `{detail}`"));
        }
    }
    Ok(())
}

fn validate_window(value: &Value, context: &str) -> Result<(), String> {
    exact_keys(value, context, &["before_entries", "after_entries"], &[])?;
    required_u64(value, "before_entries", context)?;
    required_u64(value, "after_entries", context)?;
    Ok(())
}

fn validate_page(value: &Value, context: &str) -> Result<(), String> {
    exact_keys(value, context, &[], &["entries", "cursor"])?;
    validate_optional_positive_integer(value, "entries", context)?;
    validate_optional_non_empty_string(value, "cursor", context)?;
    Ok(())
}

fn validate_answer_policy(value: &str) -> Result<(), String> {
    if ["evidence_or_unknown", "show_conflicts", "best_effort"].contains(&value) {
        Ok(())
    } else {
        Err(format!("unsupported answer_policy `{value}`"))
    }
}

fn action_tool_arguments(action: &Value) -> Option<(&str, &Value)> {
    if action.get("type").and_then(Value::as_str) != Some("tool_call") {
        return None;
    }
    let tool = action.get("tool").and_then(Value::as_str)?;
    let arguments = action.get("arguments")?;
    Some((tool, arguments))
}

fn exact_keys<'a>(
    value: &'a Value,
    context: &str,
    required: &[&str],
    optional: &[&str],
) -> Result<&'a Map<String, Value>, String> {
    let object = object(value, context)?;
    for key in required {
        if !object.contains_key(*key) {
            return Err(format!("{context} missing required field `{key}`"));
        }
    }
    for key in object.keys() {
        if !required.contains(&key.as_str()) && !optional.contains(&key.as_str()) {
            return Err(format!("{context} has unexpected field `{key}`"));
        }
    }
    Ok(object)
}

fn object<'a>(value: &'a Value, context: &str) -> Result<&'a Map<String, Value>, String> {
    value
        .as_object()
        .ok_or_else(|| format!("{context} must be an object"))
}

fn required_value<'a>(value: &'a Value, field: &str, context: &str) -> Result<&'a Value, String> {
    value
        .get(field)
        .ok_or_else(|| format!("{context} missing required field `{field}`"))
}

fn required_string<'a>(value: &'a Value, field: &str, context: &str) -> Result<&'a str, String> {
    required_value(value, field, context)?
        .as_str()
        .ok_or_else(|| format!("{context}.{field} must be a string"))
}

fn required_non_empty_string<'a>(
    value: &'a Value,
    field: &str,
    context: &str,
) -> Result<&'a str, String> {
    let value = required_string(value, field, context)?;
    if value.is_empty() {
        Err(format!("{context}.{field} must not be empty"))
    } else {
        Ok(value)
    }
}

fn validate_optional_non_empty_string(
    value: &Value,
    field: &str,
    context: &str,
) -> Result<(), String> {
    if value.get(field).is_some() {
        required_non_empty_string(value, field, context)?;
    }
    Ok(())
}

fn required_bool(value: &Value, field: &str, context: &str) -> Result<bool, String> {
    required_value(value, field, context)?
        .as_bool()
        .ok_or_else(|| format!("{context}.{field} must be a boolean"))
}

fn required_positive_integer(value: &Value, field: &str, context: &str) -> Result<u64, String> {
    let actual = required_u64(value, field, context)?;
    if actual == 0 {
        Err(format!("{context}.{field} must be > 0"))
    } else {
        Ok(actual)
    }
}

fn required_u64(value: &Value, field: &str, context: &str) -> Result<u64, String> {
    required_value(value, field, context)?
        .as_u64()
        .ok_or_else(|| format!("{context}.{field} must be a non-negative integer"))
}

fn validate_optional_positive_integer(
    value: &Value,
    field: &str,
    context: &str,
) -> Result<(), String> {
    if value.get(field).is_some() {
        required_positive_integer(value, field, context)?;
    }
    Ok(())
}

fn validate_string_array(value: &Value, context: &str) -> Result<(), String> {
    let Some(values) = value.as_array() else {
        return Err(format!("{context} must be an array"));
    };
    for (index, value) in values.iter().enumerate() {
        let Some(item) = value.as_str() else {
            return Err(format!("{context}[{index}] must be a string"));
        };
        if item.is_empty() {
            return Err(format!("{context}[{index}] must not be empty"));
        }
    }
    Ok(())
}

fn array_len(value: Option<&Value>) -> usize {
    value.and_then(Value::as_array).map(Vec::len).unwrap_or(0)
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

fn path_non_empty_string(value: &Value, path: &[&str]) -> bool {
    path_string(value, path).is_some_and(|actual| !actual.is_empty())
}

fn path_cursor<'a>(value: &'a Value, path: &[&str]) -> Option<(&'static str, &'a Value)> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    let object = current.as_object()?;
    let mut selected = None;
    for key in ["ref", "time", "sequence"] {
        if let Some(value) = object.get(key) {
            if selected.is_some() {
                return None;
            }
            selected = Some((key, value));
        }
    }
    match selected {
        Some(("ref" | "time", Value::String(value))) if !value.is_empty() => selected,
        Some(("sequence", value)) if value.as_u64().is_some_and(|actual| actual > 0) => selected,
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        kernel_operator_action_contract_error, kernel_operator_action_shape_error,
        kernel_operator_is_bounded_tool_call, kernel_operator_primary_refs,
    };

    #[test]
    fn bounded_tool_detection_accepts_expected_navigation_calls() {
        assert!(kernel_operator_is_bounded_tool_call(
            "kernel_near",
            &json!({
                "around": { "time": "2026-05-14T00:00:00Z" },
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

    #[test]
    fn action_shape_accepts_expected_operator_calls() {
        for action in [
            json!({
                "type": "tool_call",
                "tool": "kernel_wake",
                "arguments": {
                    "about": "about:1",
                    "intent": "continue investigation",
                    "dimensions": { "mode": "only", "include": ["agent"], "scope": "abouts", "abouts": ["about:2"] },
                    "budget": { "depth": 2, "tokens": 2400 }
                }
            }),
            json!({
                "type": "tool_call",
                "tool": "kernel_near",
                "arguments": {
                    "about": "about:1",
                    "around": { "sequence": 7 },
                    "dimensions": { "mode": "except", "exclude": ["discarded"], "scope": "all_abouts" },
                    "include": { "evidence": true, "raw_refs": false, "relations": true },
                    "limit": { "entries": 12, "tokens": 2400 },
                    "budget": { "depth": 3, "tokens": 2400 },
                    "window": { "before_entries": 6, "after_entries": 0 }
                }
            }),
            json!({
                "type": "tool_call",
                "tool": "kernel_inspect",
                "arguments": {
                    "ref": "node:1",
                    "include": {
                        "details": true,
                        "incoming": true,
                        "outgoing": true,
                        "raw": false
                    }
                }
            }),
            json!({
                "type": "tool_call",
                "tool": "kernel_ask",
                "arguments": {
                    "about": "about:1",
                    "answer_policy": "evidence_or_unknown",
                    "dimensions": { "mode": "all", "scope": "current_about" },
                    "question": "What changed?"
                }
            }),
            json!({
                "type": "stop",
                "answer_policy": "evidence_or_unknown",
                "final_refs": ["node:1"],
                "reason": "sufficient_evidence"
            }),
        ] {
            assert_eq!(kernel_operator_action_shape_error(&action), None);
        }
    }

    #[test]
    fn action_shape_rejects_invalid_dimension_semantics() {
        let action = json!({
            "type": "tool_call",
            "tool": "kernel_ask",
            "arguments": {
                "about": "about:1",
                "answer_policy": "evidence_or_unknown",
                "dimensions": { "mode": "only", "scope": "abouts" },
                "question": "What changed?"
            }
        });

        assert_eq!(
            kernel_operator_action_shape_error(&action),
            Some("action.arguments.dimensions.mode only requires include values".to_string())
        );
    }

    #[test]
    fn action_shape_rejects_ambiguous_temporal_cursor() {
        let action = json!({
            "type": "tool_call",
            "tool": "kernel_near",
            "arguments": {
                "about": "about:1",
                "around": { "ref": "node:1", "sequence": 1 },
                "dimensions": { "mode": "all", "scope": "current_about" },
                "include": { "evidence": true, "raw_refs": false, "relations": true },
                "limit": { "entries": 12, "tokens": 2400 },
                "budget": { "depth": 3, "tokens": 2400 },
                "window": { "before_entries": 6, "after_entries": 0 }
            }
        });

        assert_eq!(
            kernel_operator_action_shape_error(&action),
            Some(
                "action.arguments.around must set exactly one of ref, time, or sequence"
                    .to_string()
            )
        );
    }

    #[test]
    fn action_shape_rejects_extra_tool_argument_fields() {
        let action = json!({
            "type": "tool_call",
            "tool": "kernel_ask",
            "arguments": {
                "about": "about:1",
                "answer_policy": "evidence_or_unknown",
                "dimensions": { "mode": "all", "scope": "current_about" },
                "question": "What changed?",
                "final_refs": ["node:1"]
            }
        });

        assert_eq!(
            kernel_operator_action_shape_error(&action),
            Some("action.arguments has unexpected field `final_refs`".to_string())
        );
    }

    #[test]
    fn action_shape_rejects_extra_top_level_fields() {
        let action = json!({
            "type": "tool_call",
            "tool": "kernel_inspect",
            "arguments": {
                "ref": "node:1",
                "include": {
                    "details": true,
                    "incoming": true,
                    "outgoing": true,
                    "raw": false
                }
            },
            "confidence": "high"
        });

        assert_eq!(
            kernel_operator_action_shape_error(&action),
            Some("action has unexpected field `confidence`".to_string())
        );
    }

    #[test]
    fn action_contract_rejects_unbounded_navigation() {
        let action = json!({
            "type": "tool_call",
            "tool": "kernel_near",
            "arguments": {
                "about": "about:1",
                "around": { "ref": "node:1" },
                "dimensions": { "mode": "all", "scope": "current_about" },
                "include": { "evidence": true, "raw_refs": false, "relations": true },
                "limit": { "entries": 1000, "tokens": 2400 },
                "budget": { "depth": 3, "tokens": 2400 },
                "window": { "before_entries": 6, "after_entries": 0 }
            }
        });

        assert_eq!(
            kernel_operator_action_contract_error(&action),
            Some("unbounded or invalid tool call for `kernel_near`".to_string())
        );
    }
}
