use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use underpass_operator_evaluation_domain::{
    ContractCoverageProfile, ContractTrainingCoverageObservation,
};
use underpass_operator_shared_domain::{
    operator_action_contract_error, operator_allowed_tools_for_mode,
};

pub struct JsonContractCoverageObserver;

impl JsonContractCoverageObserver {
    pub fn observe(
        rows: &[Value],
        profile: ContractCoverageProfile,
    ) -> ContractTrainingCoverageObservation {
        observed_capabilities(rows, profile)
    }
}

type Profile = ContractCoverageProfile;
type ObservedCoverage = ContractTrainingCoverageObservation;

fn observed_capabilities(rows: &[Value], profile: Profile) -> ObservedCoverage {
    let mut observed = ObservedCoverage::default();
    for row in rows {
        observed.rows_total += 1;
        if let Some(action) = row.get("target_action") {
            if !has_valid_operator_mode(row) {
                record_row_parse_failure(
                    row,
                    "missing or unsupported operator mode",
                    &mut observed,
                );
                continue;
            }
            if !has_valid_allowed_tools(row) {
                record_row_parse_failure(row, "missing or invalid allowed_tools", &mut observed);
                continue;
            }
            if let Some(error) = allowed_tools_mode_error(row) {
                record_row_parse_failure(row, &error, &mut observed);
                continue;
            }
            if !profile_includes_row(profile, row) {
                observed.rows_skipped_by_profile += 1;
                continue;
            }
            observed.rows_included += 1;
            let Some(action) = resolved_action_for_coverage(row, action, &mut observed) else {
                continue;
            };
            if !validate_observed_action_contract(row, &action, &mut observed) {
                continue;
            }
            if !validate_action_allowed_by_row(row, &action, &mut observed) {
                continue;
            }
            observe_trajectory(row, &mut observed);
            observe_prepared_action(row.get("target_action").unwrap_or(&action), &mut observed);
            observe_action(&action, &mut observed);
            continue;
        }
        let Some((model_visible_row, model_action)) = model_facing_row_parts(row) else {
            record_row_parse_failure(
                row,
                "unable to parse model-facing messages/action",
                &mut observed,
            );
            continue;
        };
        if !has_valid_operator_mode(&model_visible_row) {
            record_row_parse_failure(row, "missing or unsupported operator mode", &mut observed);
            continue;
        }
        if !has_valid_allowed_tools(&model_visible_row) {
            record_row_parse_failure(row, "missing or invalid allowed_tools", &mut observed);
            continue;
        }
        if let Some(error) = allowed_tools_mode_error(&model_visible_row) {
            record_row_parse_failure(row, &error, &mut observed);
            continue;
        }
        if !profile_includes_row(profile, &model_visible_row) {
            observed.rows_skipped_by_profile += 1;
            continue;
        }
        observed.rows_included += 1;
        let Some(action) =
            resolved_action_for_coverage(&model_visible_row, &model_action, &mut observed)
        else {
            continue;
        };
        if !validate_observed_action_contract(&model_visible_row, &action, &mut observed) {
            continue;
        }
        if !validate_action_allowed_by_row(&model_visible_row, &action, &mut observed) {
            continue;
        }
        observe_trajectory(&model_visible_row, &mut observed);
        observe_prepared_action(&model_action, &mut observed);
        observe_action(&action, &mut observed);
    }
    observed
}

fn record_row_parse_failure(row: &Value, error: &str, observed: &mut ObservedCoverage) {
    observed.row_parse_failures += 1;
    if observed.row_parse_failure_examples.len() < 10 {
        let row_id = row
            .get("step_id")
            .or_else(|| row.get("id"))
            .and_then(Value::as_str)
            .unwrap_or("<unknown>");
        observed
            .row_parse_failure_examples
            .push(format!("{row_id}: {error}"));
    }
}

fn has_valid_operator_mode(row: &Value) -> bool {
    matches!(
        row.get("mode").and_then(Value::as_str),
        Some("read" | "write_context_read" | "write")
    )
}

fn has_valid_allowed_tools(row: &Value) -> bool {
    let Some(tools) = row.get("allowed_tools").and_then(Value::as_array) else {
        return false;
    };
    let mut seen = BTreeSet::new();
    for tool in tools {
        let Some(tool) = tool.as_str().filter(|tool| !tool.is_empty()) else {
            return false;
        };
        if !seen.insert(tool) {
            return false;
        }
    }
    true
}

fn allowed_tools_mode_error(row: &Value) -> Option<String> {
    let mode = row.get("mode").and_then(Value::as_str)?;
    let allowed_for_mode = operator_allowed_tools_for_mode(mode)?
        .into_iter()
        .collect::<BTreeSet<_>>();
    let tools = row.get("allowed_tools").and_then(Value::as_array)?;
    let unsupported = tools
        .iter()
        .filter_map(Value::as_str)
        .filter(|tool| !allowed_for_mode.contains(*tool))
        .map(str::to_string)
        .collect::<Vec<_>>();
    if unsupported.is_empty() {
        None
    } else {
        Some(format!(
            "allowed_tools outside mode `{mode}`: {}",
            unsupported.join(",")
        ))
    }
}

fn resolved_action_for_coverage(
    row: &Value,
    action: &Value,
    observed: &mut ObservedCoverage,
) -> Option<Value> {
    if action.get("type").and_then(Value::as_str) != Some("prepared_tool_call") {
        return Some(action.clone());
    }
    let Some(resolved) = effective_action_for_coverage(action, row) else {
        record_action_contract_failure(
            row,
            "prepared_tool_call could not be resolved to executable tool_call",
            observed,
        );
        return None;
    };
    Some(resolved)
}

fn validate_observed_action_contract(
    row: &Value,
    action: &Value,
    observed: &mut ObservedCoverage,
) -> bool {
    if let Some(error) = operator_action_contract_error(action) {
        record_action_contract_failure(row, &error, observed);
        return false;
    }
    true
}

fn validate_action_allowed_by_row(
    row: &Value,
    action: &Value,
    observed: &mut ObservedCoverage,
) -> bool {
    if !matches!(
        action.get("type").and_then(Value::as_str),
        Some("tool_call" | "prepared_tool_call")
    ) {
        return true;
    }
    let Some(tool) = action.get("tool").and_then(Value::as_str) else {
        record_action_contract_failure(row, "action.tool is missing", observed);
        return false;
    };
    let Some(allowed_tools) = row.get("allowed_tools").and_then(Value::as_array) else {
        return true;
    };
    let allowed = allowed_tools
        .iter()
        .filter_map(Value::as_str)
        .any(|allowed| allowed == tool);
    if allowed {
        true
    } else {
        record_action_contract_failure(
            row,
            &format!("tool `{tool}` is not allowed by row allowed_tools"),
            observed,
        );
        false
    }
}

fn record_action_contract_failure(row: &Value, error: &str, observed: &mut ObservedCoverage) {
    observed.action_contract_failures += 1;
    if observed.action_contract_failure_examples.len() < 10 {
        let row_id = row
            .get("step_id")
            .or_else(|| row.get("id"))
            .and_then(Value::as_str)
            .unwrap_or("<unknown>");
        observed
            .action_contract_failure_examples
            .push(format!("{row_id}: {error}"));
    }
}

fn profile_includes_row(profile: Profile, row: &Value) -> bool {
    row.get("mode")
        .and_then(Value::as_str)
        .is_some_and(|mode| profile.includes_mode(mode))
}

fn model_facing_row_parts(row: &Value) -> Option<(Value, Value)> {
    let messages = row.get("messages")?.as_array()?;
    if messages.len() != 3 {
        return None;
    }
    for (message, expected_role) in messages.iter().zip(["system", "user", "assistant"]) {
        if message.get("role").and_then(Value::as_str) != Some(expected_role)
            || !message.get("content").is_some_and(Value::is_string)
        {
            return None;
        }
    }
    let user_content = messages[1].get("content")?.as_str()?;
    let assistant_content = messages[2].get("content")?.as_str()?;
    let mut model_visible_row = serde_json::from_str::<Value>(user_content).ok()?;
    if let Value::Object(model_visible) = &mut model_visible_row {
        for key in ["step_id", "id"] {
            if !model_visible.contains_key(key)
                && let Some(value) = row.get(key).cloned()
            {
                model_visible.insert(key.to_string(), value);
            }
        }
    }
    let assistant = serde_json::from_str::<Value>(assistant_content).ok()?;
    let action = assistant.get("action")?.clone();
    Some((model_visible_row, action))
}

fn effective_action_for_coverage(action: &Value, visible_row: &Value) -> Option<Value> {
    if action.get("type").and_then(Value::as_str) != Some("prepared_tool_call") {
        return Some(action.clone());
    }
    let tool = action.get("tool")?.as_str()?;
    let source = action.get("source")?.as_str()?;
    let arguments = match (tool, source) {
        ("kernel_write_memory", "draft_write.prepared_arguments") => {
            visible_row.pointer("/visible_state/draft_write/prepared_arguments")?
        }
        ("kernel_ingest", "canonical_payload") => {
            visible_row.pointer("/visible_state/canonical_payload")?
        }
        _ => return None,
    };
    if !arguments.is_object() {
        return None;
    }
    if let (Some(row_about), Some(payload_about)) = (
        visible_row.get("about").and_then(Value::as_str),
        arguments.get("about").and_then(Value::as_str),
    ) && row_about != payload_about
    {
        return None;
    }
    Some(serde_json::json!({
        "type": "tool_call",
        "tool": tool,
        "arguments": arguments
    }))
}

fn observe_trajectory(row: &Value, observed: &mut ObservedCoverage) {
    if row.get("mode").and_then(Value::as_str) == Some("write_context_read") {
        observed.capabilities.insert("mode:write_context_read");
        observe_writer_pre_read_state(row, observed);
    } else if row.get("mode").and_then(Value::as_str) == Some("write") {
        observed.capabilities.insert("mode:write");
    }
}

fn observe_prepared_action(action: &Value, observed: &mut ObservedCoverage) {
    if action.get("type").and_then(Value::as_str) != Some("prepared_tool_call") {
        return;
    }
    match (
        action.get("tool").and_then(Value::as_str),
        action.get("source").and_then(Value::as_str),
    ) {
        (Some("kernel_write_memory"), Some("draft_write.prepared_arguments")) => {
            observed
                .capabilities
                .insert("prepared.source:draft_write.prepared_arguments");
        }
        (Some("kernel_ingest"), Some("canonical_payload")) => {
            observed
                .capabilities
                .insert("prepared.source:canonical_payload");
        }
        _ => {}
    }
}

fn observe_writer_pre_read_state(row: &Value, observed: &mut ObservedCoverage) {
    let Some(state) = row.get("visible_state") else {
        return;
    };
    match state.get("last_tool").and_then(Value::as_str) {
        Some("kernel_near") => {
            observed.capabilities.insert("writer.last_tool:kernel_near");
        }
        Some("kernel_inspect") => {
            observed
                .capabilities
                .insert("writer.last_tool:kernel_inspect");
        }
        Some("kernel_trace") => {
            observed
                .capabilities
                .insert("writer.last_tool:kernel_trace");
        }
        None if state.get("last_tool").is_some_and(Value::is_null) => {
            observed.capabilities.insert("writer.last_tool:none");
        }
        _ => {}
    }
    if state.get("candidate_pool").and_then(Value::as_str) == Some("ambiguous") {
        observed
            .capabilities
            .insert("writer.candidate_pool:ambiguous");
    }
    let Some(candidate_details) = state.get("candidate_ref_details").and_then(Value::as_array)
    else {
        return;
    };
    for detail in candidate_details {
        match detail.get("role").and_then(Value::as_str) {
            Some("previous_subtask_answer") => {
                observed
                    .capabilities
                    .insert("writer.candidate_role:previous_subtask_answer");
            }
            Some("same_subtask_question") => {
                observed
                    .capabilities
                    .insert("writer.candidate_role:same_subtask_question");
            }
            _ => {}
        }
    }
}

fn observe_action(action: &Value, observed: &mut ObservedCoverage) {
    match action.get("type").and_then(Value::as_str) {
        Some("stop") => {
            observed.capabilities.insert("tool:stop");
            observed.capabilities.insert("window:stop_sufficient");
            *observed.target_tools.entry("stop".to_string()).or_default() += 1;
            observe_answer_policy(action.get("answer_policy"), observed);
        }
        Some("tool_call") => {}
        _ => return,
    }
    let Some(tool) = action.get("tool").and_then(Value::as_str) else {
        return;
    };
    *observed.target_tools.entry(tool.to_string()).or_default() += 1;
    if let Some(capability) = match_tool_capability(tool) {
        observed.capabilities.insert(capability);
    }
    let Some(arguments) = action.get("arguments") else {
        return;
    };
    observe_dimensions(arguments, observed);
    observe_budget(arguments, observed);
    match tool {
        "kernel_ask" => observe_answer_policy(arguments.get("answer_policy"), observed),
        "kernel_near" => {
            observe_cursor(arguments.get("around"), observed);
            observe_temporal_raw_refs(arguments, observed);
        }
        "kernel_goto" => {
            observe_cursor(arguments.get("at"), observed);
            observe_temporal_raw_refs(arguments, observed);
        }
        "kernel_rewind" | "kernel_forward" => {
            observe_cursor(arguments.get("from"), observed);
            observe_temporal_raw_refs(arguments, observed);
        }
        "kernel_trace" => observe_trace_page(arguments, observed),
        "kernel_inspect" => {
            observe_inspect_raw(arguments, observed);
            if arguments.pointer("/include/raw").and_then(Value::as_bool) == Some(false) {
                observed.capabilities.insert("inspect.raw:false");
            }
        }
        "kernel_ingest" => observe_ingest(arguments, observed),
        "kernel_write_memory" => observe_write_memory(arguments, observed),
        _ => {}
    }
    observe_window(arguments, observed);
}

fn observe_answer_policy(value: Option<&Value>, observed: &mut ObservedCoverage) {
    let Some(policy) = value.and_then(Value::as_str) else {
        return;
    };
    *observed
        .answer_policies
        .entry(policy.to_string())
        .or_default() += 1;
}

fn match_tool_capability(tool: &str) -> Option<&'static str> {
    match tool {
        "kernel_wake" => Some("tool:kernel_wake"),
        "kernel_ask" => Some("tool:kernel_ask"),
        "kernel_near" => Some("tool:kernel_near"),
        "kernel_goto" => Some("tool:kernel_goto"),
        "kernel_rewind" => Some("tool:kernel_rewind"),
        "kernel_forward" => Some("tool:kernel_forward"),
        "kernel_trace" => Some("tool:kernel_trace"),
        "kernel_inspect" => Some("tool:kernel_inspect"),
        "kernel_ingest" => Some("tool:kernel_ingest"),
        "kernel_write_memory" => Some("tool:kernel_write_memory"),
        _ => None,
    }
}

fn observe_dimensions(arguments: &Value, observed: &mut ObservedCoverage) {
    let Some(dimensions) = arguments.get("dimensions") else {
        return;
    };
    if let Some(mode) = dimensions.get("mode").and_then(Value::as_str) {
        *observed
            .dimension_modes
            .entry(mode.to_string())
            .or_default() += 1;
        match mode {
            "all" => {
                observed.capabilities.insert("dimensions.mode:all");
            }
            "only" => {
                observed.capabilities.insert("dimensions.mode:only");
            }
            "except" => {
                observed.capabilities.insert("dimensions.mode:except");
            }
            _ => {}
        }
    }
    if let Some(scope) = dimensions.get("scope").and_then(Value::as_str) {
        *observed
            .dimension_scopes
            .entry(scope.to_string())
            .or_default() += 1;
        match scope {
            "current_about" => {
                observed
                    .capabilities
                    .insert("dimensions.scope:current_about");
            }
            "abouts" => {
                observed.capabilities.insert("dimensions.scope:abouts");
            }
            "all_abouts" => {
                observed.capabilities.insert("dimensions.scope:all_abouts");
            }
            _ => {}
        }
    }
    let scope_id_mode = dimensions
        .get("scope_ids")
        .and_then(Value::as_array)
        .filter(|values| !values.is_empty())
        .map(|_| "present")
        .unwrap_or("absent");
    *observed
        .dimension_scope_ids
        .entry(scope_id_mode.to_string())
        .or_default() += 1;
}

fn observe_budget(arguments: &Value, observed: &mut ObservedCoverage) {
    let Some(budget) = arguments.get("budget") else {
        return;
    };
    let detail = budget
        .get("detail")
        .and_then(Value::as_str)
        .unwrap_or("unspecified");
    *observed
        .budget_details
        .entry(detail.to_string())
        .or_default() += 1;
}

fn observe_cursor(cursor: Option<&Value>, observed: &mut ObservedCoverage) {
    let Some(cursor) = cursor else {
        return;
    };
    let mode = if cursor.get("ref").is_some() {
        Some("ref")
    } else if cursor.get("time").is_some() {
        Some("time")
    } else if cursor.get("sequence").is_some() {
        Some("sequence")
    } else {
        None
    };
    if let Some(mode) = mode {
        *observed.cursor_modes.entry(mode.to_string()).or_default() += 1;
        match mode {
            "ref" => {
                observed.capabilities.insert("cursor:ref");
            }
            "time" => {
                observed.capabilities.insert("cursor:time");
            }
            "sequence" => {
                observed.capabilities.insert("cursor:sequence");
            }
            _ => {}
        }
    }
}

fn observe_trace_page(arguments: &Value, observed: &mut ObservedCoverage) {
    let Some(page) = arguments.get("page") else {
        return;
    };
    if page.get("cursor").is_some() {
        *observed
            .trace_page_modes
            .entry("continue".to_string())
            .or_default() += 1;
        observed.capabilities.insert("trace.page:continue");
    } else {
        *observed
            .trace_page_modes
            .entry("first".to_string())
            .or_default() += 1;
        observed.capabilities.insert("trace.page:first");
    }
}

fn observe_temporal_raw_refs(arguments: &Value, observed: &mut ObservedCoverage) {
    let raw_refs = arguments
        .pointer("/include/raw_refs")
        .and_then(Value::as_bool)
        .map(|value| if value { "true" } else { "false" })
        .unwrap_or("absent");
    *observed
        .temporal_raw_refs
        .entry(raw_refs.to_string())
        .or_default() += 1;
}

fn observe_inspect_raw(arguments: &Value, observed: &mut ObservedCoverage) {
    let raw = arguments
        .pointer("/include/raw")
        .and_then(Value::as_bool)
        .map(|value| if value { "true" } else { "false" })
        .unwrap_or("absent");
    *observed.inspect_raw.entry(raw.to_string()).or_default() += 1;
}

fn observe_window(arguments: &Value, observed: &mut ObservedCoverage) {
    let before = arguments
        .pointer("/window/before_entries")
        .and_then(Value::as_u64);
    let entries = arguments.pointer("/limit/entries").and_then(Value::as_u64);
    if before.is_some_and(|value| value > 6) || entries.is_some_and(|value| value > 12) {
        observed.capabilities.insert("window:expand");
    }
    if before.is_some_and(|value| value < 6) || entries.is_some_and(|value| value < 12) {
        observed.capabilities.insert("window:shrink");
    }
}

fn observe_write_memory(arguments: &Value, observed: &mut ObservedCoverage) {
    count_label(
        &mut observed.write_memory_options,
        presence_label(arguments.get("options")),
    );
    count_label(
        &mut observed.write_memory_dry_run,
        bool_label(arguments.pointer("/options/dry_run")),
    );
    count_label(
        &mut observed.write_memory_strict,
        bool_label(arguments.pointer("/options/strict")),
    );
    count_label(
        &mut observed.write_memory_idempotency_key,
        presence_label(arguments.get("idempotency_key")),
    );
    count_label(
        &mut observed.write_memory_read_context,
        presence_label(arguments.get("read_context")),
    );
    count_label(
        &mut observed.write_memory_current_evidence,
        presence_label(arguments.pointer("/current/evidence")),
    );
    count_label(
        &mut observed.write_memory_source_kind,
        presence_label(arguments.get("source_kind")),
    );
    observe_write_memory_relation_proof(arguments, observed);

    let relation_refs = arguments
        .get("connect_to")
        .and_then(Value::as_array)
        .map(|links| {
            links
                .iter()
                .filter_map(|link| link.get("ref").and_then(Value::as_str))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let relation_quality_observed = arguments
        .get("connect_to")
        .and_then(Value::as_array)
        .is_some_and(|links| {
            !links.is_empty()
                && links.iter().all(|link| {
                    ["ref", "rel", "class", "why", "evidence"]
                        .iter()
                        .all(|field| {
                            link.get(*field)
                                .and_then(Value::as_str)
                                .is_some_and(|value| !value.is_empty())
                        })
                })
        });
    if relation_quality_observed {
        observed.capabilities.insert("write:relation_quality");
    }

    let read_context_refs = write_read_context_refs(arguments.get("read_context"));
    if relation_refs
        .iter()
        .any(|relation_ref| read_context_refs.contains(*relation_ref))
    {
        observed.capabilities.insert("write:read_context_proof");
    }
}

fn observe_write_memory_relation_proof(arguments: &Value, observed: &mut ObservedCoverage) {
    let Some(links) = arguments.get("connect_to").and_then(Value::as_array) else {
        return;
    };
    for link in links {
        let class = link
            .get("class")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let has_why = link
            .get("why")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty());
        let has_evidence = link
            .get("evidence")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty());
        let label = match (class, has_why && has_evidence) {
            ("structural", true) => "structural_with_proof",
            ("structural", false) => "structural_without_proof",
            (_, true) => "non_structural_complete",
            (_, false) => "non_structural_incomplete",
        };
        count_label(&mut observed.write_memory_relation_proof, label);
    }
}

fn observe_ingest(arguments: &Value, observed: &mut ObservedCoverage) {
    count_label(
        &mut observed.ingest_dry_run,
        bool_label(arguments.get("dry_run")),
    );
    count_label(
        &mut observed.ingest_dimensions,
        array_shape_label(arguments.pointer("/memory/dimensions")),
    );
    count_label(
        &mut observed.ingest_relations,
        array_shape_label(arguments.pointer("/memory/relations")),
    );
    count_label(
        &mut observed.ingest_evidence,
        array_shape_label(arguments.pointer("/memory/evidence")),
    );
    count_label(
        &mut observed.ingest_provenance,
        presence_label(arguments.get("provenance")),
    );
}

fn write_read_context_refs(read_context: Option<&Value>) -> BTreeSet<&str> {
    let mut refs = BTreeSet::new();
    let Some(read_context) = read_context else {
        return refs;
    };
    for field in ["inspected_refs", "temporal_refs", "wake_refs", "ask_refs"] {
        if let Some(values) = read_context.get(field).and_then(Value::as_array) {
            for value in values {
                if let Some(ref_id) = value.as_str() {
                    refs.insert(ref_id);
                }
            }
        }
    }
    if let Some(paths) = read_context.get("trace_paths").and_then(Value::as_array) {
        for path in paths {
            for field in ["from", "to"] {
                if let Some(ref_id) = path.get(field).and_then(Value::as_str) {
                    refs.insert(ref_id);
                }
            }
            if let Some(values) = path.get("refs").and_then(Value::as_array) {
                for value in values {
                    if let Some(ref_id) = value.as_str() {
                        refs.insert(ref_id);
                    }
                }
            }
        }
    }
    refs
}

fn count_label(counter: &mut BTreeMap<String, usize>, label: &'static str) {
    *counter.entry(label.to_string()).or_default() += 1;
}

fn presence_label(value: Option<&Value>) -> &'static str {
    if value.is_some() { "present" } else { "absent" }
}

fn bool_label(value: Option<&Value>) -> &'static str {
    match value {
        Some(Value::Bool(true)) => "true",
        Some(Value::Bool(false)) => "false",
        Some(_) => "invalid",
        None => "absent",
    }
}

fn array_shape_label(value: Option<&Value>) -> &'static str {
    match value {
        Some(Value::Array(values)) if values.is_empty() => "empty",
        Some(Value::Array(_)) => "non_empty",
        Some(_) => "invalid",
        None => "absent",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observes_valid_read_tool_coverage() {
        let rows = vec![serde_json::json!({
            "step_id": "read-inspect",
            "mode": "read",
            "allowed_tools": ["kernel_inspect"],
            "target_action": {
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
            }
        })];

        let observation =
            JsonContractCoverageObserver::observe(&rows, ContractCoverageProfile::Read);

        assert_eq!(observation.rows_total, 1);
        assert_eq!(observation.rows_included, 1);
        assert_eq!(observation.row_parse_failures, 0);
        assert!(observation.capabilities.contains("tool:kernel_inspect"));
        assert!(observation.capabilities.contains("inspect.raw:false"));
    }

    #[test]
    fn rejects_rows_with_tools_outside_mode_before_counting_coverage() {
        let rows = vec![serde_json::json!({
            "step_id": "bad-row",
            "mode": "read",
            "allowed_tools": ["kernel_write_memory"],
            "target_action": {
                "type": "stop",
                "answer_policy": "evidence_or_unknown",
                "final_refs": [],
                "reason": "sufficient_evidence"
            }
        })];

        let observation =
            JsonContractCoverageObserver::observe(&rows, ContractCoverageProfile::Read);

        assert_eq!(observation.rows_total, 1);
        assert_eq!(observation.rows_included, 0);
        assert_eq!(observation.row_parse_failures, 1);
        assert_eq!(
            observation.row_parse_failure_examples,
            vec!["bad-row: allowed_tools outside mode `read`: kernel_write_memory"]
        );
        assert!(observation.capabilities.is_empty());
    }
}
