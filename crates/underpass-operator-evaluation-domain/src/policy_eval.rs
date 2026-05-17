use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use serde_json::{Value, json};
use underpass_operator_shared_domain::{
    OperatorActionContractViolationPhase, operator_action_contract_diagnostic,
    operator_allowed_tools_for_mode, operator_primary_refs,
};

pub const POLICY_EVALUATOR: &str = "kernel-operator-policy-eval-v1";
pub const POLICY_ACTION_VALIDATOR: &str = "kernel-operator-action-contract-v1";
pub const POLICY_SCHEMA_MODE: &str = "strict-no-additional-properties";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyEvalBaseline {
    Deterministic,
    Oracle,
}

#[derive(Debug, Clone)]
pub struct PolicyEvalTrajectory {
    pub step_id: String,
    pub about: String,
    pub mode: String,
    pub task_family: String,
    pub allowed_tools: Option<BTreeSet<String>>,
    pub visible_state: Value,
    pub target_action: Value,
}

#[derive(Debug, Serialize)]
pub struct PolicyEvalSummary {
    pub evaluator: &'static str,
    pub action_validator: &'static str,
    pub schema_mode: &'static str,
    pub target_source: String,
    pub generated_at_unix_seconds: u64,
    pub trajectories: String,
    pub predictions: Option<String>,
    pub predictor: String,
    pub prepared_payload_resolution: bool,
    pub total: usize,
    pub by_mode: BTreeMap<String, usize>,
    pub by_mode_eval: BTreeMap<String, PolicyEvalBreakdown>,
    pub by_task_family: BTreeMap<String, usize>,
    pub target_actions: BTreeMap<String, usize>,
    pub predicted_actions: BTreeMap<String, usize>,
    pub invalid_prediction_reasons: BTreeMap<String, usize>,
    pub invalid_prediction_contract_phases: BTreeMap<String, usize>,
    pub counts: PolicyEvalCounts,
    pub rates: PolicyEvalRates,
}

#[derive(Debug, Serialize)]
pub struct PolicyEvalResult {
    pub summary: PolicyEvalSummary,
    pub details: Vec<PolicyEvalDetail>,
}

#[derive(Debug, Serialize)]
pub struct PolicyEvalDetail {
    pub step_id: String,
    pub mode: String,
    pub task_family: String,
    pub target_capability_key: String,
    pub target_action_label: String,
    pub predicted_action_label: Option<String>,
    pub prediction_status: &'static str,
    pub invalid_reason: Option<String>,
    pub invalid_contract_phase: Option<String>,
    pub score: PolicyActionScore,
}

#[derive(Debug, Serialize)]
pub struct PolicyEvalBreakdown {
    pub total: usize,
    pub counts: PolicyEvalCounts,
    pub rates: PolicyEvalRates,
}

#[derive(Debug, Default, Serialize)]
pub struct PolicyActionScore {
    pub action_type_correct: bool,
    pub tool_correct: bool,
    pub primary_refs_correct: bool,
    pub scope_correct: bool,
    pub cursor_mode_correct: bool,
    pub window_shape_correct: bool,
    pub limit_policy_correct: bool,
    pub continue_page_correct: bool,
    pub stop_correct: bool,
    pub exact_action_correct: bool,
}

#[derive(Debug, Default, Serialize)]
pub struct PolicyEvalCounts {
    pub target_tool_calls: usize,
    pub target_stop_actions: usize,
    pub target_cursor_actions: usize,
    pub target_window_actions: usize,
    pub target_limit_actions: usize,
    pub target_page_continuations: usize,
    pub missing_predictions: usize,
    pub invalid_predictions: usize,
    pub unbounded_tool_calls: usize,
    pub action_type_correct: usize,
    pub tool_correct: usize,
    pub primary_refs_correct: usize,
    pub scope_correct: usize,
    pub cursor_mode_correct: usize,
    pub window_shape_correct: usize,
    pub limit_policy_correct: usize,
    pub continue_page_correct: usize,
    pub stop_correct: usize,
    pub exact_action_correct: usize,
}

#[derive(Debug, Default, Serialize)]
pub struct PolicyEvalRates {
    pub action_type_accuracy: f64,
    pub tool_accuracy: f64,
    pub primary_ref_accuracy: f64,
    pub scope_accuracy: f64,
    pub cursor_mode_accuracy: f64,
    pub window_shape_accuracy: f64,
    pub limit_policy_accuracy: f64,
    pub continue_page_accuracy: f64,
    pub stop_accuracy: f64,
    pub exact_action_accuracy: f64,
    pub invalid_prediction_rate: f64,
    pub unbounded_tool_call_rate: f64,
}

#[derive(Debug, Clone)]
pub struct PolicyEvalRequest {
    pub target_source: String,
    pub trajectories_label: String,
    pub predictions_label: Option<String>,
    pub predictor: String,
    pub generated_at_unix_seconds: u64,
    pub baseline: PolicyEvalBaseline,
    pub resolve_prepared_payloads: bool,
    pub trajectories: Vec<PolicyEvalTrajectory>,
    pub predictions: Option<BTreeMap<String, Value>>,
}

pub struct PolicyEvaluator;

impl PolicyEvaluator {
    pub fn evaluate(request: PolicyEvalRequest) -> Result<PolicyEvalResult, String> {
        evaluate_internal(&request)
    }
}

fn evaluate_internal(request: &PolicyEvalRequest) -> Result<PolicyEvalResult, String> {
    let trajectories = request.trajectories.as_slice();
    let predictions = request.predictions.as_ref();

    if !request.resolve_prepared_payloads
        && trajectories
            .iter()
            .any(|trajectory| action_type(&trajectory.target_action) == Some("prepared_tool_call"))
    {
        return Err(
            "model-facing prepared payload targets require --resolve-prepared-payloads".to_string(),
        );
    }

    let mut counts = PolicyEvalCounts::default();
    let mut by_mode = BTreeMap::<String, usize>::new();
    let mut by_mode_counts = BTreeMap::<String, PolicyEvalCounts>::new();
    let mut by_task_family = BTreeMap::<String, usize>::new();
    let mut target_actions = BTreeMap::<String, usize>::new();
    let mut predicted_actions = BTreeMap::<String, usize>::new();
    let mut invalid_prediction_reasons = BTreeMap::<String, usize>::new();
    let mut invalid_prediction_contract_phases = BTreeMap::<String, usize>::new();
    let mut details = Vec::<PolicyEvalDetail>::new();

    for raw_trajectory in trajectories {
        let mut trajectory = raw_trajectory.clone();
        if let Some(allowed_tools) = &trajectory.allowed_tools {
            validate_allowed_tools_for_mode(
                &trajectory.mode,
                allowed_tools,
                &format!("trajectory `{}`", trajectory.step_id),
            )?;
        }
        if request.resolve_prepared_payloads {
            trajectory.target_action = resolve_prepared_payload_action(
                &raw_trajectory.target_action,
                &raw_trajectory.visible_state,
                &raw_trajectory.about,
            )
            .map_err(|error| {
                format!(
                    "failed to resolve target prepared payload for `{}`: {error}",
                    raw_trajectory.step_id
                )
            })?;
        }
        if let Some(error) = action_allowed_tools_error(&trajectory, &trajectory.target_action) {
            return Err(format!(
                "target action for `{}` violates row allowed_tools: {error}",
                trajectory.step_id
            ));
        }
        let target_diagnostic = operator_action_contract_diagnostic(&trajectory.target_action);
        if let Some(violation) = target_diagnostic.violation() {
            return Err(format!(
                "target action for `{}` violates KMP action contract: {}",
                trajectory.step_id,
                violation.message()
            ));
        }
        let mode_key = trajectory.mode.clone();
        *by_mode.entry(mode_key.clone()).or_default() += 1;
        *by_task_family
            .entry(trajectory.task_family.clone())
            .or_default() += 1;
        *target_actions
            .entry(action_label(&trajectory.target_action))
            .or_default() += 1;
        count_target_metrics(&trajectory.target_action, &mut counts);
        count_target_metrics(
            &trajectory.target_action,
            by_mode_counts.entry(mode_key.clone()).or_default(),
        );

        let predicted = predicted_action(request.baseline, &trajectory, predictions);
        let Some(predicted) = predicted else {
            counts.missing_predictions += 1;
            by_mode_counts
                .entry(mode_key)
                .or_default()
                .missing_predictions += 1;
            details.push(eval_detail(
                &trajectory,
                None,
                "missing",
                None,
                None,
                PolicyActionScore::default(),
            ));
            continue;
        };
        let predicted = if request.resolve_prepared_payloads {
            match resolve_prepared_payload_action(
                &predicted,
                &trajectory.visible_state,
                &trajectory.about,
            ) {
                Ok(resolved) => resolved,
                Err(error) => {
                    counts.invalid_predictions += 1;
                    by_mode_counts
                        .entry(mode_key)
                        .or_default()
                        .invalid_predictions += 1;
                    let reason = format!("prepared payload resolution failed: {error}");
                    *invalid_prediction_reasons
                        .entry(reason.clone())
                        .or_default() += 1;
                    details.push(eval_detail(
                        &trajectory,
                        Some(&predicted),
                        "invalid",
                        Some(reason),
                        None,
                        PolicyActionScore::default(),
                    ));
                    continue;
                }
            }
        } else {
            predicted
        };
        let predicted_action_label = action_label(&predicted);
        *predicted_actions.entry(predicted_action_label).or_default() += 1;
        if let Some(error) = action_allowed_tools_error(&trajectory, &predicted) {
            counts.invalid_predictions += 1;
            by_mode_counts
                .entry(mode_key)
                .or_default()
                .invalid_predictions += 1;
            *invalid_prediction_reasons.entry(error.clone()).or_default() += 1;
            details.push(eval_detail(
                &trajectory,
                Some(&predicted),
                "invalid",
                Some(error),
                None,
                PolicyActionScore::default(),
            ));
            continue;
        }
        let diagnostic = operator_action_contract_diagnostic(&predicted);
        if let Some(violation) = diagnostic.violation() {
            counts.invalid_predictions += 1;
            by_mode_counts
                .entry(mode_key)
                .or_default()
                .invalid_predictions += 1;
            let phase = violation.phase();
            if phase == OperatorActionContractViolationPhase::ToolBounds {
                counts.unbounded_tool_calls += 1;
                by_mode_counts
                    .entry(trajectory.mode.clone())
                    .or_default()
                    .unbounded_tool_calls += 1;
            }
            let error = violation.message().to_string();
            count_contract_phase(&mut invalid_prediction_contract_phases, phase);
            *invalid_prediction_reasons.entry(error.clone()).or_default() += 1;
            details.push(eval_detail(
                &trajectory,
                Some(&predicted),
                "invalid",
                Some(error),
                Some(phase.as_str().to_string()),
                PolicyActionScore::default(),
            ));
            continue;
        }
        let score = action_score(&trajectory.target_action, &predicted);
        apply_score(&trajectory.target_action, &predicted, &score, &mut counts);
        apply_score(
            &trajectory.target_action,
            &predicted,
            &score,
            by_mode_counts.entry(mode_key).or_default(),
        );
        details.push(eval_detail(
            &trajectory,
            Some(&predicted),
            "valid",
            None,
            None,
            score,
        ));
    }

    let summary_rates = rates(&counts, trajectories.len());
    let by_mode_eval = by_mode_counts
        .into_iter()
        .map(|(mode, counts)| {
            let total = by_mode.get(&mode).copied().unwrap_or_default();
            let rates = rates(&counts, total);
            (
                mode,
                PolicyEvalBreakdown {
                    total,
                    counts,
                    rates,
                },
            )
        })
        .collect();
    Ok(PolicyEvalResult {
        summary: PolicyEvalSummary {
            evaluator: POLICY_EVALUATOR,
            action_validator: POLICY_ACTION_VALIDATOR,
            schema_mode: POLICY_SCHEMA_MODE,
            target_source: request.target_source.clone(),
            generated_at_unix_seconds: request.generated_at_unix_seconds,
            trajectories: request.trajectories_label.clone(),
            predictions: request.predictions_label.clone(),
            predictor: request.predictor.clone(),
            prepared_payload_resolution: request.resolve_prepared_payloads,
            total: trajectories.len(),
            by_mode,
            by_mode_eval,
            by_task_family,
            target_actions,
            predicted_actions,
            invalid_prediction_reasons,
            invalid_prediction_contract_phases,
            counts,
            rates: summary_rates,
        },
        details,
    })
}

pub fn policy_deterministic_action(trajectory: &PolicyEvalTrajectory) -> Value {
    deterministic_action(trajectory)
}

pub fn policy_action_type(action: &Value) -> Option<&str> {
    action_type(action)
}

pub fn policy_tool(action: &Value) -> Option<&str> {
    tool(action)
}

fn eval_detail(
    trajectory: &PolicyEvalTrajectory,
    predicted: Option<&Value>,
    prediction_status: &'static str,
    invalid_reason: Option<String>,
    invalid_contract_phase: Option<String>,
    score: PolicyActionScore,
) -> PolicyEvalDetail {
    PolicyEvalDetail {
        step_id: trajectory.step_id.clone(),
        mode: trajectory.mode.clone(),
        task_family: trajectory.task_family.clone(),
        target_capability_key: target_capability_key(trajectory),
        target_action_label: action_label(&trajectory.target_action),
        predicted_action_label: predicted.map(action_label),
        prediction_status,
        invalid_reason,
        invalid_contract_phase,
        score,
    }
}

fn predicted_action(
    baseline: PolicyEvalBaseline,
    trajectory: &PolicyEvalTrajectory,
    predictions: Option<&BTreeMap<String, Value>>,
) -> Option<Value> {
    if let Some(predictions) = predictions {
        return predictions.get(&trajectory.step_id).cloned();
    }
    match baseline {
        PolicyEvalBaseline::Oracle => Some(trajectory.target_action.clone()),
        PolicyEvalBaseline::Deterministic => Some(deterministic_action(trajectory)),
    }
}

fn action_allowed_tools_error(trajectory: &PolicyEvalTrajectory, action: &Value) -> Option<String> {
    if !matches!(
        action_type(action),
        Some("tool_call" | "prepared_tool_call")
    ) {
        return None;
    }
    let Some(allowed_tools) = &trajectory.allowed_tools else {
        return None;
    };
    let tool = tool(action).unwrap_or("<missing>");
    if allowed_tools.contains(tool) {
        None
    } else {
        Some(format!("tool `{tool}` is not allowed by row allowed_tools"))
    }
}

fn validate_allowed_tools_for_mode(
    mode: &str,
    allowed_tools: &BTreeSet<String>,
    location: &str,
) -> Result<(), String> {
    let allowed_for_mode = operator_allowed_tools_for_mode(mode)
        .ok_or_else(|| format!("{location} unsupported operator mode `{mode}`"))?
        .into_iter()
        .collect::<BTreeSet<_>>();
    let unsupported = allowed_tools
        .iter()
        .filter(|tool| !allowed_for_mode.contains(*tool))
        .cloned()
        .collect::<Vec<_>>();
    if unsupported.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{location} allowed_tools outside mode `{mode}`: {}",
            unsupported.join(",")
        ))
    }
}

fn deterministic_action(trajectory: &PolicyEvalTrajectory) -> Value {
    let current_ref = trajectory
        .visible_state
        .get("current_ref")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let last_tool = trajectory
        .visible_state
        .get("last_tool")
        .and_then(Value::as_str);
    let trace_target_ref = trajectory
        .visible_state
        .get("trace_target_ref")
        .and_then(Value::as_str);

    match last_tool {
        None => json!({
            "type": "tool_call",
            "tool": "kernel_near",
            "arguments": {
                "about": trajectory.about,
                "around": { "ref": current_ref },
                "dimensions": { "mode": "all", "scope": "current_about" },
                "include": { "evidence": true, "raw_refs": false, "relations": true },
                "limit": { "entries": 12, "tokens": 2400 },
                "budget": { "depth": 3, "tokens": 2400 },
                "window": { "before_entries": 6, "after_entries": 0 }
            }
        }),
        Some("kernel_near") => json!({
            "type": "tool_call",
            "tool": "kernel_inspect",
            "arguments": {
                "ref": current_ref,
                "include": {
                    "details": true,
                    "incoming": true,
                    "outgoing": true,
                    "raw": false
                }
            }
        }),
        Some("kernel_inspect") if trace_target_ref.is_some() => json!({
            "type": "tool_call",
            "tool": "kernel_trace",
            "arguments": {
                "from": current_ref,
                "to": trace_target_ref,
                "goal": "Kernel operator deterministic trace probe",
                "budget": { "depth": 1, "tokens": 1600 }
            }
        }),
        Some("kernel_trace")
        | Some("kernel_inspect")
        | Some("kernel_goto")
        | Some("kernel_rewind")
        | Some("kernel_forward")
        | Some("kernel_ask")
        | Some(_) => {
            json!({
                "type": "stop",
                "answer_policy": "evidence_or_unknown",
                "final_refs": trajectory.visible_state.get("known_refs").cloned().unwrap_or_else(|| json!([])),
                "reason": "deterministic_baseline_stop"
            })
        }
    }
}

fn action_score(target: &Value, predicted: &Value) -> PolicyActionScore {
    let mut score = PolicyActionScore::default();
    let target_type = action_type(target);
    let predicted_type = action_type(predicted);
    if target_type == predicted_type {
        score.action_type_correct = true;
    }
    if target_type == Some("stop")
        && predicted_type == Some("stop")
        && stop_matches(target, predicted)
    {
        score.stop_correct = true;
    }
    if target == predicted {
        score.exact_action_correct = true;
    }
    if target_type == Some("tool_call") && predicted_type == Some("tool_call") {
        if tool(target) == tool(predicted) {
            score.tool_correct = true;
        }
        if operator_primary_refs(target) == operator_primary_refs(predicted) {
            score.primary_refs_correct = true;
        }
        if scope_about(target) == scope_about(predicted) {
            score.scope_correct = true;
        }
        if target_cursor_mode(target).is_some()
            && target_cursor_mode(target) == target_cursor_mode(predicted)
        {
            score.cursor_mode_correct = true;
        }
        if action_window(target).is_some() && action_window(target) == action_window(predicted) {
            score.window_shape_correct = true;
        }
        if action_limit(target).is_some() && action_limit(target) == action_limit(predicted) {
            score.limit_policy_correct = true;
        }
        if trace_page_cursor(target).is_some()
            && trace_page_cursor(target) == trace_page_cursor(predicted)
        {
            score.continue_page_correct = true;
        }
    }
    score
}

fn apply_score(
    target: &Value,
    _predicted: &Value,
    score: &PolicyActionScore,
    counts: &mut PolicyEvalCounts,
) {
    if score.action_type_correct {
        counts.action_type_correct += 1;
    }
    if score.stop_correct {
        counts.stop_correct += 1;
    }
    if score.exact_action_correct {
        counts.exact_action_correct += 1;
    }
    if action_type(target) == Some("tool_call") {
        if score.tool_correct {
            counts.tool_correct += 1;
        }
        if score.primary_refs_correct {
            counts.primary_refs_correct += 1;
        }
        if score.scope_correct {
            counts.scope_correct += 1;
        }
        if score.cursor_mode_correct {
            counts.cursor_mode_correct += 1;
        }
        if score.window_shape_correct {
            counts.window_shape_correct += 1;
        }
        if score.limit_policy_correct {
            counts.limit_policy_correct += 1;
        }
        if score.continue_page_correct {
            counts.continue_page_correct += 1;
        }
    }
}

fn stop_matches(target: &Value, predicted: &Value) -> bool {
    target.get("answer_policy") == predicted.get("answer_policy")
        && target.get("final_refs") == predicted.get("final_refs")
}

fn count_target_metrics(target: &Value, counts: &mut PolicyEvalCounts) {
    match action_type(target) {
        Some("tool_call") => counts.target_tool_calls += 1,
        Some("stop") => counts.target_stop_actions += 1,
        _ => {}
    }
    count_target_navigation_metrics(target, counts);
}

fn count_target_navigation_metrics(target: &Value, counts: &mut PolicyEvalCounts) {
    if target_cursor_mode(target).is_some() {
        counts.target_cursor_actions += 1;
    }
    if action_window(target).is_some() {
        counts.target_window_actions += 1;
    }
    if action_limit(target).is_some() {
        counts.target_limit_actions += 1;
    }
    if trace_page_cursor(target).is_some() {
        counts.target_page_continuations += 1;
    }
}

fn rates(counts: &PolicyEvalCounts, total: usize) -> PolicyEvalRates {
    PolicyEvalRates {
        action_type_accuracy: ratio(counts.action_type_correct, total),
        tool_accuracy: ratio(counts.tool_correct, counts.target_tool_calls),
        primary_ref_accuracy: ratio(counts.primary_refs_correct, counts.target_tool_calls),
        scope_accuracy: ratio(counts.scope_correct, counts.target_tool_calls),
        cursor_mode_accuracy: ratio(counts.cursor_mode_correct, counts.target_cursor_actions),
        window_shape_accuracy: ratio(counts.window_shape_correct, counts.target_window_actions),
        limit_policy_accuracy: ratio(counts.limit_policy_correct, counts.target_limit_actions),
        continue_page_accuracy: ratio(
            counts.continue_page_correct,
            counts.target_page_continuations,
        ),
        stop_accuracy: ratio(counts.stop_correct, counts.target_stop_actions),
        exact_action_accuracy: ratio(counts.exact_action_correct, total),
        invalid_prediction_rate: ratio(counts.invalid_predictions, total),
        unbounded_tool_call_rate: ratio(counts.unbounded_tool_calls, total),
    }
}

fn ratio(count: usize, total: usize) -> f64 {
    if total == 0 {
        return 0.0;
    }
    count as f64 / total as f64
}

fn action_label(action: &Value) -> String {
    match action_type(action) {
        Some("tool_call") => format!("tool_call:{}", tool(action).unwrap_or("unknown")),
        Some(kind) => kind.to_string(),
        None => "invalid".to_string(),
    }
}

fn target_capability_key(trajectory: &PolicyEvalTrajectory) -> String {
    let action = &trajectory.target_action;
    let label = action_label(action);
    let cursor = target_cursor_mode(action).unwrap_or("none");
    let dimension_mode = dimension_mode(action).unwrap_or("none");
    let dimension_scope = dimension_scope(action).unwrap_or("none");
    let trace_page = trace_page_mode(action);
    format!(
        "{}|{}|cursor:{cursor}|dim_mode:{dimension_mode}|dim_scope:{dimension_scope}|trace_page:{trace_page}",
        trajectory.task_family, label
    )
}

fn count_contract_phase(
    phases: &mut BTreeMap<String, usize>,
    phase: OperatorActionContractViolationPhase,
) {
    *phases.entry(phase.as_str().to_string()).or_default() += 1;
}

fn action_type(action: &Value) -> Option<&str> {
    action.get("type").and_then(Value::as_str)
}

fn tool(action: &Value) -> Option<&str> {
    action.get("tool").and_then(Value::as_str)
}

fn scope_about(action: &Value) -> Option<String> {
    action
        .get("arguments")
        .and_then(|arguments| arguments.get("about"))
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn target_cursor_mode(action: &Value) -> Option<&'static str> {
    let arguments = action.get("arguments")?;
    let cursor_key = match tool(action)? {
        "kernel_near" => "around",
        "kernel_goto" => "at",
        "kernel_rewind" | "kernel_forward" => "from",
        _ => return None,
    };
    let cursor = arguments.get(cursor_key)?.as_object()?;
    if cursor.contains_key("ref") {
        Some("ref")
    } else if cursor.contains_key("time") {
        Some("time")
    } else if cursor.contains_key("sequence") {
        Some("sequence")
    } else {
        None
    }
}

fn action_window(action: &Value) -> Option<&Value> {
    action.get("arguments")?.get("window")
}

fn action_limit(action: &Value) -> Option<&Value> {
    action.get("arguments")?.get("limit")
}

fn dimension_mode(action: &Value) -> Option<&str> {
    action
        .get("arguments")?
        .get("dimensions")?
        .get("mode")?
        .as_str()
}

fn dimension_scope(action: &Value) -> Option<&str> {
    action
        .get("arguments")?
        .get("dimensions")?
        .get("scope")?
        .as_str()
}

fn trace_page_mode(action: &Value) -> &'static str {
    if tool(action) != Some("kernel_trace") {
        return "none";
    }
    let Some(page) = action
        .get("arguments")
        .and_then(|arguments| arguments.get("page"))
    else {
        return "none";
    };
    if page.get("cursor").is_some() {
        "continue"
    } else {
        "first"
    }
}

fn trace_page_cursor(action: &Value) -> Option<&str> {
    if tool(action) != Some("kernel_trace") {
        return None;
    }
    action
        .get("arguments")?
        .get("page")?
        .get("cursor")?
        .as_str()
}

fn resolve_prepared_payload_action(
    action: &Value,
    visible_state: &Value,
    about: &str,
) -> Result<Value, String> {
    if action_type(action) != Some("prepared_tool_call") {
        return Ok(action.clone());
    }
    exact_action_keys(action, &["type", "tool", "source"])?;
    let tool = tool(action).ok_or_else(|| "missing tool".to_string())?;
    let source = action
        .get("source")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing source".to_string())?;
    let arguments = match (tool, source) {
        ("kernel_write_memory", "draft_write.prepared_arguments") => visible_state
            .pointer("/draft_write/prepared_arguments")
            .ok_or_else(|| "missing draft_write.prepared_arguments".to_string())?,
        ("kernel_ingest", "canonical_payload") => visible_state
            .get("canonical_payload")
            .ok_or_else(|| "missing canonical_payload".to_string())?,
        _ => {
            return Err(format!(
                "unsupported prepared payload source `{source}` for tool `{tool}`"
            ));
        }
    };
    if !arguments.is_object() {
        return Err("prepared payload is not an object".to_string());
    }
    if arguments.get("about").and_then(Value::as_str) != Some(about) {
        return Err("prepared payload about does not match trajectory about".to_string());
    }
    let resolved = json!({
        "type": "tool_call",
        "tool": tool,
        "arguments": arguments
    });
    let diagnostic = operator_action_contract_diagnostic(&resolved);
    if let Some(violation) = diagnostic.violation() {
        return Err(format!(
            "resolved action violates KMP action contract: {}",
            violation.message()
        ));
    }
    Ok(resolved)
}

fn exact_action_keys(action: &Value, expected: &[&str]) -> Result<(), String> {
    let object = action
        .as_object()
        .ok_or_else(|| "prepared action is not an object".to_string())?;
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    let missing = expected.difference(&actual).copied().collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(format!(
            "prepared action missing required field(s): {}",
            missing.join(",")
        ));
    }
    let unexpected = actual.difference(&expected).copied().collect::<Vec<_>>();
    if !unexpected.is_empty() {
        return Err(format!(
            "prepared action has unexpected field(s): {}",
            unexpected.join(",")
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oracle_baseline_scores_exact_actions() {
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
            }
        });
        let request = PolicyEvalRequest {
            target_source: "raw_trajectories".to_string(),
            trajectories_label: "trajectories.jsonl".to_string(),
            predictions_label: None,
            predictor: "baseline:oracle".to_string(),
            generated_at_unix_seconds: 1,
            baseline: PolicyEvalBaseline::Oracle,
            resolve_prepared_payloads: false,
            trajectories: vec![PolicyEvalTrajectory {
                step_id: "s1".to_string(),
                about: "about:1".to_string(),
                mode: "read".to_string(),
                task_family: "conformance.inspect".to_string(),
                allowed_tools: None,
                visible_state: json!({}),
                target_action: action,
            }],
            predictions: None,
        };

        let result = PolicyEvaluator::evaluate(request).expect("policy result");

        assert_eq!(result.summary.counts.exact_action_correct, 1);
        assert_eq!(result.summary.counts.tool_correct, 1);
        assert_eq!(result.summary.rates.exact_action_accuracy, 1.0);
    }
}
