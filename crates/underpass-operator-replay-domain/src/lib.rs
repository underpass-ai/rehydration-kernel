use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use underpass_operator_shared_domain::{
    OperatorActionContractViolationPhase, operator_action_contract_diagnostic,
};

pub const MCP_REPLAYER: &str = "kernel-operator-mcp-replay-v1";

#[derive(Debug, Clone, Deserialize)]
pub struct ReplayTrajectory {
    pub step_id: String,
    pub about: String,
    pub mode: String,
    pub task_family: String,
    pub allowed_tools: Vec<String>,
    #[serde(default)]
    pub observed_outcome: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct ReplayPrediction {
    pub action: Value,
}

#[derive(Debug, Clone)]
pub struct ReplayToolCallPlan {
    pub tool: String,
    pub arguments: Value,
    pub action_label: String,
    pub expected_observed_refs: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum ReplayActionDecision {
    MissingPrediction(ReplayRow),
    InvalidPrediction {
        row: ReplayRow,
        unbounded_tool_call: bool,
        contract_violation_phase: Option<String>,
    },
    Stop(ReplayRow),
    ToolCall(ReplayToolCallPlan),
}

#[derive(Debug, Clone, Serialize)]
pub struct ReplayRow {
    pub step_id: String,
    pub about: String,
    pub mode: String,
    pub task_family: String,
    pub action_label: String,
    pub elapsed_ms: u128,
    pub success: bool,
    pub error: Option<String>,
    pub contract_violation_phase: Option<String>,
    pub partial_result: bool,
    pub page: Option<ReplayPage>,
    pub expected_observed_refs: Vec<String>,
    pub observed_refs: Vec<String>,
    pub missing_expected_refs: Vec<String>,
    pub extra_observed_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReplayPage {
    pub returned: Option<u64>,
    pub total: Option<u64>,
    pub has_more: bool,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReplaySummary {
    pub replayer: &'static str,
    pub generated_at_unix_seconds: u64,
    pub endpoint: String,
    pub trajectories: String,
    pub predictions: String,
    pub output: String,
    pub selected: usize,
    pub tool_calls: usize,
    pub stop_actions: usize,
    pub executed_tool_calls: usize,
    pub successful_tool_calls: usize,
    pub failed_tool_calls: usize,
    pub missing_predictions: usize,
    pub invalid_predictions: usize,
    pub unbounded_tool_calls: usize,
    pub contract_violation_phases: BTreeMap<String, usize>,
    pub missing_expected_ref_rows: usize,
    pub missing_expected_ref_total: usize,
    pub extra_observed_ref_rows: usize,
    pub extra_observed_ref_total: usize,
    pub partial_result_rows: usize,
    pub partial_result_by_action: BTreeMap<String, usize>,
    pub by_action: BTreeMap<String, usize>,
    pub latency_ms_by_action: BTreeMap<String, ActionLatencySummary>,
    pub elapsed_ms: u128,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActionLatencySummary {
    pub count: usize,
    pub avg_ms: f64,
    pub max_ms: u128,
}

#[derive(Debug, Clone)]
pub struct ReplaySummaryMetadata {
    pub generated_at_unix_seconds: u64,
    pub endpoint: String,
    pub trajectories: String,
    pub predictions: String,
    pub output: String,
    pub elapsed_ms: u128,
}

#[derive(Debug, Default, Clone)]
pub struct ReplayCounters {
    pub tool_calls: usize,
    pub stop_actions: usize,
    pub executed_tool_calls: usize,
    pub successful_tool_calls: usize,
    pub failed_tool_calls: usize,
    pub missing_predictions: usize,
    pub invalid_predictions: usize,
    pub unbounded_tool_calls: usize,
    pub contract_violation_phases: BTreeMap<String, usize>,
    pub missing_expected_ref_rows: usize,
    pub missing_expected_ref_total: usize,
}

impl ReplayCounters {
    pub fn record_missing_prediction(&mut self) {
        self.missing_predictions += 1;
    }

    pub fn record_invalid_prediction(
        &mut self,
        unbounded_tool_call: bool,
        contract_violation_phase: Option<&str>,
    ) {
        if unbounded_tool_call {
            self.unbounded_tool_calls += 1;
        } else {
            self.invalid_predictions += 1;
        }
        if let Some(phase) = contract_violation_phase {
            *self
                .contract_violation_phases
                .entry(phase.to_string())
                .or_default() += 1;
        }
    }

    pub fn record_stop(&mut self, row: &ReplayRow) {
        self.stop_actions += 1;
        self.record_missing_refs(row);
    }

    pub fn record_tool_call_started(&mut self) {
        self.tool_calls += 1;
        self.executed_tool_calls += 1;
    }

    pub fn record_tool_call_result(&mut self, row: &ReplayRow) {
        if row.success {
            self.successful_tool_calls += 1;
        } else {
            self.failed_tool_calls += 1;
            self.record_missing_refs(row);
        }
    }

    fn record_missing_refs(&mut self, row: &ReplayRow) {
        if !row.missing_expected_refs.is_empty() {
            self.missing_expected_ref_rows += 1;
            self.missing_expected_ref_total += row.missing_expected_refs.len();
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ReplayRunReport {
    pub summary: ReplaySummary,
    pub rows: Vec<ReplayRow>,
}

pub fn decide_replay_action(
    trajectory: &ReplayTrajectory,
    prediction: Option<&ReplayPrediction>,
) -> ReplayActionDecision {
    let expected_observed_refs = expected_observed_refs(trajectory);
    let Some(prediction) = prediction else {
        return ReplayActionDecision::MissingPrediction(failed_row(
            trajectory,
            "invalid".to_string(),
            "missing_prediction".to_string(),
            expected_observed_refs,
        ));
    };

    let action = &prediction.action;
    let label = action_label(action);
    let diagnostic = operator_action_contract_diagnostic(action);
    if let Some(violation) = diagnostic.violation() {
        let phase = violation.phase();
        let phase_label = phase.as_str().to_string();
        return ReplayActionDecision::InvalidPrediction {
            row: failed_row_with_contract_phase(
                trajectory,
                label,
                format!("action_contract:{}", violation.message()),
                Some(phase_label.clone()),
                expected_observed_refs,
            ),
            unbounded_tool_call: phase == OperatorActionContractViolationPhase::ToolBounds,
            contract_violation_phase: Some(phase_label),
        };
    }
    if let Some(tool) = tool(action)
        && !trajectory
            .allowed_tools
            .iter()
            .any(|allowed_tool| allowed_tool == tool)
    {
        return ReplayActionDecision::InvalidPrediction {
            row: failed_row(
                trajectory,
                label,
                format!("tool_not_allowed:{tool}"),
                expected_observed_refs,
            ),
            unbounded_tool_call: false,
            contract_violation_phase: None,
        };
    }

    match action_type(action) {
        Some("stop") => ReplayActionDecision::Stop(stop_row(
            trajectory,
            label,
            expected_observed_refs,
            stop_final_refs(action),
        )),
        Some("tool_call") => {
            let Some(tool) = tool(action) else {
                return ReplayActionDecision::InvalidPrediction {
                    row: failed_row(
                        trajectory,
                        label,
                        "tool_call_missing_tool".to_string(),
                        expected_observed_refs,
                    ),
                    unbounded_tool_call: false,
                    contract_violation_phase: None,
                };
            };
            let Some(arguments) = action.get("arguments") else {
                return ReplayActionDecision::InvalidPrediction {
                    row: failed_row(
                        trajectory,
                        label,
                        "tool_call_missing_arguments".to_string(),
                        expected_observed_refs,
                    ),
                    unbounded_tool_call: false,
                    contract_violation_phase: None,
                };
            };
            ReplayActionDecision::ToolCall(ReplayToolCallPlan {
                tool: tool.to_string(),
                arguments: arguments.clone(),
                action_label: label,
                expected_observed_refs,
            })
        }
        Some(other) => ReplayActionDecision::InvalidPrediction {
            row: failed_row(
                trajectory,
                "invalid".to_string(),
                format!("unsupported_action_type:{other}"),
                expected_observed_refs,
            ),
            unbounded_tool_call: false,
            contract_violation_phase: None,
        },
        None => ReplayActionDecision::InvalidPrediction {
            row: failed_row(
                trajectory,
                "invalid".to_string(),
                "missing_action_type".to_string(),
                expected_observed_refs,
            ),
            unbounded_tool_call: false,
            contract_violation_phase: None,
        },
    }
}

pub fn tool_success_row(
    trajectory: &ReplayTrajectory,
    plan: &ReplayToolCallPlan,
    elapsed_ms: u128,
    content: &Value,
) -> ReplayRow {
    let page = page_from_content(content);
    let partial_result = page.as_ref().is_some_and(|page| page.has_more);
    let observed_refs = collect_memory_refs(content).into_iter().collect::<Vec<_>>();
    let (missing_expected_refs, extra_observed_refs) =
        ref_differences(&plan.expected_observed_refs, &observed_refs);
    ReplayRow {
        step_id: trajectory.step_id.clone(),
        about: trajectory.about.clone(),
        mode: trajectory.mode.clone(),
        task_family: trajectory.task_family.clone(),
        action_label: plan.action_label.clone(),
        elapsed_ms,
        success: missing_expected_refs.is_empty(),
        error: if missing_expected_refs.is_empty() {
            None
        } else {
            Some("missing_expected_refs".to_string())
        },
        contract_violation_phase: None,
        partial_result,
        page,
        expected_observed_refs: plan.expected_observed_refs.clone(),
        observed_refs,
        missing_expected_refs,
        extra_observed_refs,
    }
}

pub fn tool_error_row(
    trajectory: &ReplayTrajectory,
    plan: &ReplayToolCallPlan,
    error: String,
) -> ReplayRow {
    failed_row(
        trajectory,
        plan.action_label.clone(),
        error,
        plan.expected_observed_refs.clone(),
    )
}

pub fn build_replay_summary(
    metadata: ReplaySummaryMetadata,
    selected: usize,
    counters: ReplayCounters,
    rows: &[ReplayRow],
) -> ReplaySummary {
    ReplaySummary {
        replayer: MCP_REPLAYER,
        generated_at_unix_seconds: metadata.generated_at_unix_seconds,
        endpoint: metadata.endpoint,
        trajectories: metadata.trajectories,
        predictions: metadata.predictions,
        output: metadata.output,
        selected,
        tool_calls: counters.tool_calls,
        stop_actions: counters.stop_actions,
        executed_tool_calls: counters.executed_tool_calls,
        successful_tool_calls: counters.successful_tool_calls,
        failed_tool_calls: counters.failed_tool_calls,
        missing_predictions: counters.missing_predictions,
        invalid_predictions: counters.invalid_predictions,
        unbounded_tool_calls: counters.unbounded_tool_calls,
        contract_violation_phases: counters.contract_violation_phases,
        missing_expected_ref_rows: counters.missing_expected_ref_rows,
        missing_expected_ref_total: counters.missing_expected_ref_total,
        extra_observed_ref_rows: rows
            .iter()
            .filter(|row| !row.extra_observed_refs.is_empty())
            .count(),
        extra_observed_ref_total: rows.iter().map(|row| row.extra_observed_refs.len()).sum(),
        partial_result_rows: rows.iter().filter(|row| row.partial_result).count(),
        partial_result_by_action: partial_result_by_action(rows),
        by_action: by_action(rows),
        latency_ms_by_action: latency_ms_by_action(rows),
        elapsed_ms: metadata.elapsed_ms,
    }
}

fn failed_row(
    trajectory: &ReplayTrajectory,
    action_label: String,
    error: String,
    expected_observed_refs: Vec<String>,
) -> ReplayRow {
    ReplayRow {
        step_id: trajectory.step_id.clone(),
        about: trajectory.about.clone(),
        mode: trajectory.mode.clone(),
        task_family: trajectory.task_family.clone(),
        action_label,
        elapsed_ms: 0,
        success: false,
        error: Some(error),
        contract_violation_phase: None,
        partial_result: false,
        page: None,
        expected_observed_refs,
        observed_refs: Vec::new(),
        missing_expected_refs: Vec::new(),
        extra_observed_refs: Vec::new(),
    }
}

fn failed_row_with_contract_phase(
    trajectory: &ReplayTrajectory,
    action_label: String,
    error: String,
    contract_violation_phase: Option<String>,
    expected_observed_refs: Vec<String>,
) -> ReplayRow {
    let mut row = failed_row(trajectory, action_label, error, expected_observed_refs);
    row.contract_violation_phase = contract_violation_phase;
    row
}

fn stop_row(
    trajectory: &ReplayTrajectory,
    action_label: String,
    expected_observed_refs: Vec<String>,
    observed_refs: Vec<String>,
) -> ReplayRow {
    let (missing_expected_refs, extra_observed_refs) =
        ref_differences(&expected_observed_refs, &observed_refs);
    ReplayRow {
        step_id: trajectory.step_id.clone(),
        about: trajectory.about.clone(),
        mode: trajectory.mode.clone(),
        task_family: trajectory.task_family.clone(),
        action_label,
        elapsed_ms: 0,
        success: missing_expected_refs.is_empty(),
        error: if missing_expected_refs.is_empty() {
            None
        } else {
            Some("missing_expected_refs".to_string())
        },
        contract_violation_phase: None,
        partial_result: false,
        page: None,
        expected_observed_refs,
        observed_refs,
        missing_expected_refs,
        extra_observed_refs,
    }
}

pub fn expected_observed_refs(trajectory: &ReplayTrajectory) -> Vec<String> {
    trajectory
        .observed_outcome
        .as_ref()
        .and_then(|value| value.get("observed_refs"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(ToString::to_string)
        .collect()
}

pub fn stop_final_refs(action: &Value) -> Vec<String> {
    action
        .get("final_refs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(ToString::to_string)
        .collect()
}

pub fn ref_differences(expected: &[String], observed: &[String]) -> (Vec<String>, Vec<String>) {
    let expected_set = expected.iter().cloned().collect::<BTreeSet<_>>();
    let observed_set = observed.iter().cloned().collect::<BTreeSet<_>>();
    let missing = expected_set.difference(&observed_set).cloned().collect();
    let extra = observed_set.difference(&expected_set).cloned().collect();
    (missing, extra)
}

pub fn page_from_content(content: &Value) -> Option<ReplayPage> {
    let page = content.get("page")?;
    let object = page.as_object()?;
    let next_cursor = object
        .get("next_cursor")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string);

    Some(ReplayPage {
        returned: object.get("returned").and_then(Value::as_u64),
        total: object.get("total").and_then(Value::as_u64),
        has_more: object
            .get("has_more")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        next_cursor,
    })
}

pub fn collect_memory_refs(value: &Value) -> BTreeSet<String> {
    let mut refs = BTreeSet::new();
    collect_memory_refs_from_field(value, None, &mut refs);
    refs
}

fn collect_memory_refs_from_field(value: &Value, field: Option<&str>, refs: &mut BTreeSet<String>) {
    match value {
        Value::String(value) if field_allows_memory_ref(field) && looks_like_memory_ref(value) => {
            refs.insert(value.to_string());
        }
        Value::Array(values) => {
            for value in values {
                collect_memory_refs_from_field(value, field, refs);
            }
        }
        Value::Object(object) => {
            for (key, value) in object {
                collect_memory_refs_from_field(value, Some(key), refs);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn field_allows_memory_ref(field: Option<&str>) -> bool {
    let Some(field) = field else {
        return false;
    };
    matches!(
        field,
        "anchor"
            | "because"
            | "cursor"
            | "evidence"
            | "from"
            | "node"
            | "path"
            | "proof"
            | "ref"
            | "reference"
            | "references"
            | "refs"
            | "related"
            | "source"
            | "sources"
            | "supports"
            | "target"
            | "to"
            | "trace"
    ) || field.ends_with("_ref")
        || field.ends_with("_refs")
        || field.ends_with("_ref_id")
        || field.ends_with("_ref_ids")
}

fn looks_like_memory_ref(value: &str) -> bool {
    !value.contains(' ')
        && value.len() <= 500
        && (value.starts_with("memoryarena:")
            || value.starts_with("memoryagentbench:")
            || value.starts_with("longmemeval:")
            || value.starts_with("incident:")
            || value.starts_with("turn:")
            || value.starts_with("question:")
            || value.starts_with("evidence:")
            || value.starts_with("about:"))
}

pub fn action_label(action: &Value) -> String {
    match action_type(action) {
        Some("tool_call") => format!("tool_call:{}", tool(action).unwrap_or("unknown")),
        Some(kind) => kind.to_string(),
        None => "invalid".to_string(),
    }
}

pub fn action_type(action: &Value) -> Option<&str> {
    action.get("type").and_then(Value::as_str)
}

pub fn tool(action: &Value) -> Option<&str> {
    action.get("tool").and_then(Value::as_str)
}

fn by_action(rows: &[ReplayRow]) -> BTreeMap<String, usize> {
    let mut by_action = BTreeMap::new();
    for row in rows {
        *by_action.entry(row.action_label.clone()).or_default() += 1;
    }
    by_action
}

fn latency_ms_by_action(rows: &[ReplayRow]) -> BTreeMap<String, ActionLatencySummary> {
    let mut by_action = BTreeMap::<String, LatencyDraft>::new();
    for row in rows {
        let draft = by_action.entry(row.action_label.clone()).or_default();
        draft.count += 1;
        draft.total_ms += row.elapsed_ms;
        draft.max_ms = draft.max_ms.max(row.elapsed_ms);
    }
    by_action
        .into_iter()
        .map(|(action, draft)| {
            let avg_ms = if draft.count == 0 {
                0.0
            } else {
                draft.total_ms as f64 / draft.count as f64
            };
            (
                action,
                ActionLatencySummary {
                    count: draft.count,
                    avg_ms,
                    max_ms: draft.max_ms,
                },
            )
        })
        .collect()
}

fn partial_result_by_action(rows: &[ReplayRow]) -> BTreeMap<String, usize> {
    let mut by_action = BTreeMap::new();
    for row in rows.iter().filter(|row| row.partial_result) {
        *by_action.entry(row.action_label.clone()).or_default() += 1;
    }
    by_action
}

#[derive(Debug, Default)]
struct LatencyDraft {
    count: usize,
    total_ms: u128,
    max_ms: u128,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn collect_memory_refs_accepts_benchmark_and_conformance_refs() {
        let refs = collect_memory_refs(&json!({
            "proof": {
                "evidence": [{
                    "supports": [
                        "memoryarena:run:demo:task:1",
                        "memoryagentbench:split:conflict_resolution:source:fact:item:item-1",
                        "incident:writer-pre-read-v3:demo:subtask:1:answer",
                        "turn:run:lme:question:q:answer:a:1",
                        "evidence:run:lme:question:q:answer:a:1"
                    ],
                    "source": "question:run:lme:question:q"
                }],
                "path": [{
                    "from": "about:longmemeval:run:lme:item:q:dimension:longmemeval:session:s1",
                    "to": "longmemeval:run:lme:item:q"
                }]
            },
            "answer": "mentions turn:run:lme:question:q:answer:a:2 but answer text is not proof"
        }));

        assert!(refs.contains("memoryarena:run:demo:task:1"));
        assert!(
            refs.contains("memoryagentbench:split:conflict_resolution:source:fact:item:item-1")
        );
        assert!(refs.contains("incident:writer-pre-read-v3:demo:subtask:1:answer"));
        assert!(refs.contains("turn:run:lme:question:q:answer:a:1"));
        assert!(refs.contains("evidence:run:lme:question:q:answer:a:1"));
        assert!(refs.contains("question:run:lme:question:q"));
        assert!(refs.contains("about:longmemeval:run:lme:item:q:dimension:longmemeval:session:s1"));
        assert!(refs.contains("longmemeval:run:lme:item:q"));
        assert!(!refs.contains("turn:run:lme:question:q:answer:a:2"));
    }

    #[test]
    fn stop_final_refs_are_replay_observed_refs() {
        let refs = stop_final_refs(&json!({
            "type": "stop",
            "answer_policy": "evidence_or_unknown",
            "final_refs": ["incident:run:node:a", "incident:run:node:b"],
            "reason": "sufficient evidence"
        }));

        assert_eq!(
            refs,
            vec![
                "incident:run:node:a".to_string(),
                "incident:run:node:b".to_string()
            ]
        );
    }

    #[test]
    fn ref_differences_report_missing_stop_evidence() {
        let expected = vec![
            "incident:run:node:a".to_string(),
            "incident:run:node:b".to_string(),
        ];
        let observed = vec!["incident:run:node:a".to_string()];

        let (missing, extra) = ref_differences(&expected, &observed);

        assert_eq!(missing, vec!["incident:run:node:b".to_string()]);
        assert!(extra.is_empty());
    }

    #[test]
    fn invalid_contract_predictions_expose_violation_phase() {
        let trajectory = ReplayTrajectory {
            step_id: "step:1".to_string(),
            about: "incident:1".to_string(),
            mode: "read".to_string(),
            task_family: "conformance.near".to_string(),
            allowed_tools: vec!["kernel_near".to_string()],
            observed_outcome: None,
        };
        let prediction = ReplayPrediction {
            action: json!({
                "type": "tool_call",
                "tool": "kernel_near",
                "arguments": {
                    "about": "incident:1",
                    "around": {"ref": "incident:1:node:a"},
                    "dimensions": {"mode": "all", "scope": "current_about"},
                    "include": {"evidence": true, "raw_refs": false, "relations": true},
                    "limit": {"entries": 1000, "tokens": 2400},
                    "budget": {"depth": 3, "tokens": 2400},
                    "window": {"before_entries": 6, "after_entries": 0}
                }
            }),
        };

        let decision = decide_replay_action(&trajectory, Some(&prediction));

        match decision {
            ReplayActionDecision::InvalidPrediction {
                row,
                unbounded_tool_call,
                contract_violation_phase,
            } => {
                assert!(unbounded_tool_call);
                assert_eq!(contract_violation_phase.as_deref(), Some("tool_bounds"));
                assert_eq!(row.contract_violation_phase.as_deref(), Some("tool_bounds"));
                assert_eq!(
                    row.error.as_deref(),
                    Some("action_contract:unbounded or invalid tool call for `kernel_near`")
                );
            }
            other => panic!("expected invalid prediction, got {other:?}"),
        }
    }
}

#[cfg(test)]
mod dependency_tests {
    use std::fs;
    use std::path::Path;

    #[test]
    fn crate_has_no_rehydration_dependencies() {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        let contents = fs::read_to_string(manifest).expect("manifest must be readable");

        assert!(
            !contents.contains("rehydration-"),
            "underpass-operator-replay-domain must stay independent from kernel crates"
        );
    }
}
