use std::collections::BTreeMap;
#[cfg(test)]
use std::collections::BTreeSet;
use std::env;
use std::error::Error;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;
#[cfg(test)]
use serde_json::json;

use underpass_operator_evaluation_application::EvaluateOperatorPolicyUseCase;
#[cfg(test)]
use underpass_operator_evaluation_domain::{
    POLICY_ACTION_VALIDATOR as ACTION_VALIDATOR, POLICY_SCHEMA_MODE as SCHEMA_MODE,
    PolicyEvalSummary as EvalSummary, policy_action_type, policy_deterministic_action, policy_tool,
};
use underpass_operator_evaluation_domain::{
    PolicyEvalBaseline as Baseline, PolicyEvalRequest, PolicyEvalResult as EvalResult,
    PolicyEvalTrajectory as Trajectory,
};
use underpass_operator_evaluation_infra::{JsonlPolicyEvalReader, PolicyTrajectoryJsonlFormat};
#[cfg(test)]
use underpass_operator_shared_domain::operator_primary_refs as kernel_operator_primary_refs;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Args {
    trajectories: TargetInput,
    predictions: Option<PathBuf>,
    baseline: Baseline,
    resolve_prepared_payloads: bool,
    output: Option<PathBuf>,
    details_output: Option<PathBuf>,
    limit: Option<usize>,
    offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TargetInput {
    RawTrajectories(PathBuf),
    ModelFacingEval(PathBuf),
}

impl TargetInput {
    fn path(&self) -> &Path {
        match self {
            TargetInput::RawTrajectories(path) | TargetInput::ModelFacingEval(path) => path,
        }
    }

    fn source_label(&self) -> &'static str {
        match self {
            TargetInput::RawTrajectories(_) => "raw_trajectories",
            TargetInput::ModelFacingEval(_) => "model_facing_eval",
        }
    }

    fn format(&self) -> PolicyTrajectoryJsonlFormat {
        match self {
            TargetInput::RawTrajectories(_) => PolicyTrajectoryJsonlFormat::RawTrajectories,
            TargetInput::ModelFacingEval(_) => PolicyTrajectoryJsonlFormat::ModelFacingEval,
        }
    }
}

fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let args = parse_args(env::args().skip(1))?;
    let trajectories = select_trajectories(read_target_input(&args.trajectories)?, &args);
    let predictions = match args.predictions.as_deref() {
        Some(path) => Some(read_predictions(path)?),
        None => None,
    };
    let result = evaluate_internal(&args, &trajectories, predictions.as_ref())?;
    let rendered = serde_json::to_string_pretty(&result.summary)?;
    if let Some(output) = args.output.as_deref() {
        let file = File::create(output)?;
        let mut writer = BufWriter::new(file);
        writer.write_all(rendered.as_bytes())?;
        writer.write_all(b"\n")?;
        writer.flush()?;
    }
    if let Some(output) = args.details_output.as_deref() {
        let file = File::create(output)?;
        let mut writer = BufWriter::new(file);
        for detail in &result.details {
            serde_json::to_writer(&mut writer, detail)?;
            writer.write_all(b"\n")?;
        }
        writer.flush()?;
    }
    println!("{rendered}");
    Ok(())
}

fn parse_args(
    mut args: impl Iterator<Item = String>,
) -> Result<Args, Box<dyn Error + Send + Sync>> {
    let mut trajectories = None;
    let mut predictions = None;
    let mut baseline = Baseline::Deterministic;
    let mut resolve_prepared_payloads = false;
    let mut output = None;
    let mut details_output = None;
    let mut limit = None;
    let mut offset = 0usize;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--trajectories" => {
                set_target_input(
                    &mut trajectories,
                    TargetInput::RawTrajectories(PathBuf::from(next_arg(
                        &mut args,
                        "--trajectories",
                    )?)),
                    "--trajectories",
                )?;
            }
            "--model-facing-eval" => {
                set_target_input(
                    &mut trajectories,
                    TargetInput::ModelFacingEval(PathBuf::from(next_arg(
                        &mut args,
                        "--model-facing-eval",
                    )?)),
                    "--model-facing-eval",
                )?;
            }
            "--predictions" => {
                predictions = Some(PathBuf::from(next_arg(&mut args, "--predictions")?));
            }
            "--baseline" => {
                baseline = parse_baseline(&next_arg(&mut args, "--baseline")?)?;
            }
            "--resolve-prepared-payloads" => resolve_prepared_payloads = true,
            "--output" => output = Some(PathBuf::from(next_arg(&mut args, "--output")?)),
            "--details-output" => {
                details_output = Some(PathBuf::from(next_arg(&mut args, "--details-output")?));
            }
            "--limit" => limit = Some(next_arg(&mut args, "--limit")?.parse()?),
            "--offset" => offset = next_arg(&mut args, "--offset")?.parse()?,
            "--help" | "-h" => return Err(usage().into()),
            value if value.starts_with('-') => {
                return Err(format!("unknown argument: {value}\n{}", usage()).into());
            }
            value => {
                if trajectories.is_some() {
                    return Err(format!("unexpected positional argument: {value}").into());
                }
                trajectories = Some(TargetInput::RawTrajectories(PathBuf::from(value)));
            }
        }
    }

    Ok(Args {
        trajectories: trajectories.ok_or_else(usage)?,
        predictions,
        baseline,
        resolve_prepared_payloads,
        output,
        details_output,
        limit,
        offset,
    })
}

fn parse_baseline(value: &str) -> Result<Baseline, Box<dyn Error + Send + Sync>> {
    match value {
        "deterministic" => Ok(Baseline::Deterministic),
        "oracle" => Ok(Baseline::Oracle),
        other => Err(format!("unknown baseline `{other}`; expected deterministic|oracle").into()),
    }
}

fn set_target_input(
    target: &mut Option<TargetInput>,
    value: TargetInput,
    name: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    if target.is_some() {
        return Err(format!("{name} cannot be combined with another target input").into());
    }
    *target = Some(value);
    Ok(())
}

fn next_arg(
    args: &mut impl Iterator<Item = String>,
    name: &str,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    args.next()
        .ok_or_else(|| format!("{name} requires a value").into())
}

fn usage() -> String {
    "usage: underpass_operator_policy_eval (--trajectories <trajectories.jsonl> | --model-facing-eval <eval.jsonl>) [--predictions predictions.jsonl] [--baseline deterministic|oracle] [--resolve-prepared-payloads] [--output summary.json] [--details-output details.jsonl] [--limit n] [--offset n]".to_string()
}

fn read_target_input(input: &TargetInput) -> Result<Vec<Trajectory>, Box<dyn Error + Send + Sync>> {
    read_trajectories_with_format(input.path(), input.format())
}

fn read_trajectories_with_format(
    path: &Path,
    format: PolicyTrajectoryJsonlFormat,
) -> Result<Vec<Trajectory>, Box<dyn Error + Send + Sync>> {
    JsonlPolicyEvalReader::read_trajectories(path, format).map_err(|error| error.into())
}

#[cfg(test)]
fn read_trajectories(path: &Path) -> Result<Vec<Trajectory>, Box<dyn Error + Send + Sync>> {
    read_trajectories_with_format(path, PolicyTrajectoryJsonlFormat::RawTrajectories)
}

#[cfg(test)]
fn read_model_facing_eval(path: &Path) -> Result<Vec<Trajectory>, Box<dyn Error + Send + Sync>> {
    read_trajectories_with_format(path, PolicyTrajectoryJsonlFormat::ModelFacingEval)
}

fn read_predictions(path: &Path) -> Result<BTreeMap<String, Value>, Box<dyn Error + Send + Sync>> {
    JsonlPolicyEvalReader::read_predictions(path).map_err(|error| error.into())
}

fn select_trajectories(values: Vec<Trajectory>, args: &Args) -> Vec<Trajectory> {
    values
        .into_iter()
        .skip(args.offset)
        .take(args.limit.unwrap_or(usize::MAX))
        .collect()
}

#[cfg(test)]
fn evaluate(
    args: &Args,
    trajectories: &[Trajectory],
    predictions: Option<&BTreeMap<String, Value>>,
) -> Result<EvalSummary, Box<dyn Error + Send + Sync>> {
    Ok(evaluate_internal(args, trajectories, predictions)?.summary)
}

fn evaluate_internal(
    args: &Args,
    trajectories: &[Trajectory],
    predictions: Option<&BTreeMap<String, Value>>,
) -> Result<EvalResult, Box<dyn Error + Send + Sync>> {
    let request = policy_eval_request(
        args,
        trajectories.to_vec(),
        predictions.cloned(),
        now_unix_seconds()?,
    );
    EvaluateOperatorPolicyUseCase::new()
        .execute(request)
        .map_err(|error| error.into())
}

fn policy_eval_request(
    args: &Args,
    trajectories: Vec<Trajectory>,
    predictions: Option<BTreeMap<String, Value>>,
    generated_at_unix_seconds: u64,
) -> PolicyEvalRequest {
    PolicyEvalRequest {
        target_source: args.trajectories.source_label().to_string(),
        trajectories_label: args.trajectories.path().display().to_string(),
        predictions_label: args
            .predictions
            .as_ref()
            .map(|path| path.display().to_string()),
        predictor: predictor_label(args),
        generated_at_unix_seconds,
        baseline: args.baseline,
        resolve_prepared_payloads: args.resolve_prepared_payloads,
        trajectories,
        predictions,
    }
}

fn predictor_label(args: &Args) -> String {
    match args.predictions.as_ref() {
        Some(_) => "predictions".to_string(),
        None => match args.baseline {
            Baseline::Deterministic => "baseline:deterministic".to_string(),
            Baseline::Oracle => "baseline:oracle".to_string(),
        },
    }
}

fn now_unix_seconds() -> Result<u64, Box<dyn Error + Send + Sync>> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}

#[cfg(test)]
fn deterministic_action(trajectory: &Trajectory) -> Value {
    policy_deterministic_action(trajectory)
}

#[cfg(test)]
fn action_type(action: &Value) -> Option<&str> {
    policy_action_type(action)
}

#[cfg(test)]
fn tool(action: &Value) -> Option<&str> {
    policy_tool(action)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn deterministic_policy_starts_with_near() -> Result<(), Box<dyn Error + Send + Sync>> {
        let trajectory = Trajectory {
            step_id: "s1".to_string(),
            about: "memoryarena:run:r1:task_type:progressive_search:task:1".to_string(),
            mode: "read".to_string(),
            task_family: "memoryarena.progressive_search".to_string(),
            allowed_tools: None,
            visible_state: json!({
                "current_ref": "memoryarena:run:r1:task_type:progressive_search:task:1:subtask:1:question",
                "trace_target_ref": null,
                "last_tool": null,
                "known_refs": [],
            }),
            target_action: json!({ "type": "stop" }),
        };
        let action = deterministic_action(&trajectory);
        assert_eq!(action_type(&action), Some("tool_call"));
        assert_eq!(tool(&action), Some("kernel_near"));
        let refs = kernel_operator_primary_refs(&action);
        if refs != ["memoryarena:run:r1:task_type:progressive_search:task:1:subtask:1:question"] {
            return Err(format!("unexpected refs: {refs:?}").into());
        }
        Ok(())
    }

    #[test]
    fn oracle_baseline_scores_exact_actions() -> Result<(), Box<dyn Error + Send + Sync>> {
        let trajectories = vec![Trajectory {
            step_id: "s1".to_string(),
            about: "memoryarena:run:r1:task_type:progressive_search:task:1".to_string(),
            mode: "read".to_string(),
            task_family: "memoryarena.progressive_search".to_string(),
            allowed_tools: None,
            visible_state: json!({
                "current_ref": "node:1",
                "last_tool": null,
            }),
            target_action: json!({
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
        }];
        let args = Args {
            trajectories: TargetInput::RawTrajectories(PathBuf::from("trajectories.jsonl")),
            predictions: None,
            baseline: Baseline::Oracle,
            resolve_prepared_payloads: false,
            output: None,
            details_output: None,
            limit: None,
            offset: 0,
        };
        let summary = evaluate(&args, &trajectories, None)?;
        assert_eq!(summary.counts.exact_action_correct, 1);
        assert_eq!(summary.counts.tool_correct, 1);
        Ok(())
    }

    #[test]
    fn summary_reports_mode_specific_gate_metrics() -> Result<(), Box<dyn Error + Send + Sync>> {
        let read_action = json!({
            "type": "tool_call",
            "tool": "kernel_inspect",
            "arguments": {
                "ref": "node:read",
                "include": {
                    "details": true,
                    "incoming": true,
                    "outgoing": true,
                    "raw": false
                }
            }
        });
        let writer_target = json!({
            "type": "tool_call",
            "tool": "kernel_near",
            "arguments": {
                "about": "about:1",
                "around": { "ref": "node:writer" },
                "dimensions": { "mode": "all", "scope": "current_about" },
                "include": { "evidence": true, "raw_refs": false, "relations": true },
                "limit": { "entries": 12, "tokens": 2400 },
                "budget": { "depth": 3, "tokens": 2400 },
                "window": { "before_entries": 6, "after_entries": 0 }
            }
        });
        let writer_prediction = json!({
            "type": "tool_call",
            "tool": "kernel_trace",
            "arguments": {
                "from": "node:writer",
                "to": "node:prior",
                "budget": { "depth": 2, "tokens": 1600 }
            }
        });
        let trajectories = vec![
            Trajectory {
                step_id: "read".to_string(),
                about: "about:1".to_string(),
                mode: "read".to_string(),
                task_family: "memoryarena.progressive_search".to_string(),
                allowed_tools: None,
                visible_state: json!({ "current_ref": "node:read" }),
                target_action: read_action.clone(),
            },
            Trajectory {
                step_id: "writer".to_string(),
                about: "about:1".to_string(),
                mode: "write_context_read".to_string(),
                task_family: "memoryarena.smart_writer".to_string(),
                allowed_tools: None,
                visible_state: json!({ "current_ref": "node:writer" }),
                target_action: writer_target,
            },
        ];
        let predictions = BTreeMap::from([
            ("read".to_string(), read_action),
            ("writer".to_string(), writer_prediction),
        ]);
        let args = Args {
            trajectories: TargetInput::RawTrajectories(PathBuf::from("trajectories.jsonl")),
            predictions: Some(PathBuf::from("predictions.jsonl")),
            baseline: Baseline::Deterministic,
            resolve_prepared_payloads: false,
            output: None,
            details_output: None,
            limit: None,
            offset: 0,
        };

        let summary = evaluate(&args, &trajectories, Some(&predictions))?;

        assert_eq!(summary.counts.exact_action_correct, 1);
        assert_eq!(summary.rates.exact_action_accuracy, 0.5);

        let read = summary.by_mode_eval.get("read").expect("read mode summary");
        assert_eq!(read.total, 1);
        assert_eq!(read.counts.exact_action_correct, 1);
        assert_eq!(read.rates.exact_action_accuracy, 1.0);

        let writer = summary
            .by_mode_eval
            .get("write_context_read")
            .expect("writer mode summary");
        assert_eq!(writer.total, 1);
        assert_eq!(writer.counts.exact_action_correct, 0);
        assert_eq!(writer.rates.exact_action_accuracy, 0.0);
        Ok(())
    }

    #[test]
    fn unbounded_prediction_is_counted() -> Result<(), Box<dyn Error + Send + Sync>> {
        let trajectories = vec![Trajectory {
            step_id: "s1".to_string(),
            about: "memoryarena:run:r1:task_type:progressive_search:task:1".to_string(),
            mode: "read".to_string(),
            task_family: "memoryarena.progressive_search".to_string(),
            allowed_tools: None,
            visible_state: json!({
                "current_ref": "node:1",
                "last_tool": null,
            }),
            target_action: json!({
                "type": "tool_call",
                "tool": "kernel_near",
                "arguments": {
                    "about": "about:1",
                    "around": { "ref": "node:1" },
                    "dimensions": { "mode": "all", "scope": "current_about" },
                    "include": { "evidence": true, "raw_refs": false, "relations": true },
                    "limit": { "entries": 12, "tokens": 2400 },
                    "budget": { "depth": 3, "tokens": 2400 },
                    "window": { "before_entries": 6, "after_entries": 0 }
                }
            }),
        }];
        let mut predictions = BTreeMap::new();
        predictions.insert(
            "s1".to_string(),
            json!({
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
            }),
        );
        let args = Args {
            trajectories: TargetInput::RawTrajectories(PathBuf::from("trajectories.jsonl")),
            predictions: Some(PathBuf::from("predictions.jsonl")),
            baseline: Baseline::Deterministic,
            resolve_prepared_payloads: false,
            output: None,
            details_output: None,
            limit: None,
            offset: 0,
        };
        let summary = evaluate(&args, &trajectories, Some(&predictions))?;
        assert_eq!(summary.counts.invalid_predictions, 1);
        assert_eq!(summary.counts.unbounded_tool_calls, 1);
        assert_eq!(summary.counts.tool_correct, 0);
        assert_eq!(
            summary
                .invalid_prediction_reasons
                .get("unbounded or invalid tool call for `kernel_near`"),
            Some(&1)
        );
        Ok(())
    }

    #[test]
    fn invalid_prediction_reason_is_counted() -> Result<(), Box<dyn Error + Send + Sync>> {
        let trajectories = vec![Trajectory {
            step_id: "s1".to_string(),
            about: "about:1".to_string(),
            mode: "read".to_string(),
            task_family: "longmemeval.temporal-reasoning".to_string(),
            allowed_tools: None,
            visible_state: json!({
                "current_ref": "node:1",
                "last_tool": null,
            }),
            target_action: json!({
                "type": "tool_call",
                "tool": "kernel_ask",
                "arguments": {
                    "about": "about:1",
                    "answer_policy": "evidence_or_unknown",
                    "dimensions": { "mode": "all", "scope": "current_about" },
                    "question": "What changed?",
                    "budget": { "tokens": 2400 }
                }
            }),
        }];
        let mut predictions = BTreeMap::new();
        predictions.insert(
            "s1".to_string(),
            json!({
                "type": "tool_call",
                "tool": "kernel_ask",
                "arguments": {
                    "about": "about:1",
                    "answer_policy": "evidence_or_unknown",
                    "dimensions": { "mode": "all", "scope": "current_about" },
                    "question": "What changed?",
                    "budget": { "tokens": 2400 },
                    "final_refs": ["node:1"]
                }
            }),
        );
        let args = Args {
            trajectories: TargetInput::RawTrajectories(PathBuf::from("trajectories.jsonl")),
            predictions: Some(PathBuf::from("predictions.jsonl")),
            baseline: Baseline::Deterministic,
            resolve_prepared_payloads: false,
            output: None,
            details_output: None,
            limit: None,
            offset: 0,
        };
        let summary = evaluate(&args, &trajectories, Some(&predictions))?;
        assert_eq!(summary.action_validator, ACTION_VALIDATOR);
        assert_eq!(summary.schema_mode, SCHEMA_MODE);
        assert_eq!(summary.counts.invalid_predictions, 1);
        assert_eq!(
            summary
                .invalid_prediction_reasons
                .get("action.arguments has unexpected field `final_refs`"),
            Some(&1)
        );
        Ok(())
    }

    #[test]
    fn prediction_tool_must_be_allowed_by_row() -> Result<(), Box<dyn Error + Send + Sync>> {
        let trajectories = vec![Trajectory {
            step_id: "s1".to_string(),
            about: "about:1".to_string(),
            mode: "read".to_string(),
            task_family: "conformance.read.allowed_tools".to_string(),
            allowed_tools: Some(BTreeSet::from(["kernel_inspect".to_string()])),
            visible_state: json!({
                "current_ref": "node:1",
                "last_tool": null,
            }),
            target_action: json!({
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
        }];
        let predictions = BTreeMap::from([(
            "s1".to_string(),
            json!({
                "type": "tool_call",
                "tool": "kernel_near",
                "arguments": {
                    "about": "about:1",
                    "around": { "ref": "node:1" },
                    "dimensions": { "mode": "all", "scope": "current_about" },
                    "include": { "evidence": true, "raw_refs": false, "relations": true },
                    "limit": { "entries": 12, "tokens": 2400 },
                    "budget": { "depth": 3, "tokens": 2400 },
                    "window": { "before_entries": 6, "after_entries": 0 }
                }
            }),
        )]);
        let args = Args {
            trajectories: TargetInput::RawTrajectories(PathBuf::from("trajectories.jsonl")),
            predictions: Some(PathBuf::from("predictions.jsonl")),
            baseline: Baseline::Deterministic,
            resolve_prepared_payloads: false,
            output: None,
            details_output: None,
            limit: None,
            offset: 0,
        };

        let summary = evaluate(&args, &trajectories, Some(&predictions))?;

        assert_eq!(summary.counts.invalid_predictions, 1);
        assert_eq!(
            summary
                .invalid_prediction_reasons
                .get("tool `kernel_near` is not allowed by row allowed_tools"),
            Some(&1)
        );
        Ok(())
    }

    #[test]
    fn target_action_must_pass_contract() -> Result<(), Box<dyn Error + Send + Sync>> {
        let trajectories = vec![Trajectory {
            step_id: "bad-target".to_string(),
            about: "about:1".to_string(),
            mode: "read".to_string(),
            task_family: "conformance.read.invalid_target".to_string(),
            allowed_tools: None,
            visible_state: json!({}),
            target_action: json!({
                "type": "tool_call",
                "tool": "kernel_ask",
                "arguments": {
                    "about": "about:1",
                    "answer_policy": "evidence_or_unknown",
                    "dimensions": { "mode": "all", "scope": "current_about" },
                    "question": "What changed?"
                }
            }),
        }];
        let args = Args {
            trajectories: TargetInput::RawTrajectories(PathBuf::from("trajectories.jsonl")),
            predictions: None,
            baseline: Baseline::Oracle,
            resolve_prepared_payloads: false,
            output: None,
            details_output: None,
            limit: None,
            offset: 0,
        };

        let error = evaluate_internal(&args, &trajectories, None)
            .expect_err("invalid target action must fail eval");

        assert!(
            error
                .to_string()
                .contains("target action for `bad-target` violates KMP action contract")
        );
        Ok(())
    }

    #[test]
    fn stop_accuracy_requires_final_refs_and_policy() -> Result<(), Box<dyn Error + Send + Sync>> {
        let target = json!({
            "type": "stop",
            "answer_policy": "evidence_or_unknown",
            "final_refs": ["incident:run:node:a", "incident:run:node:b"],
            "reason": "sufficient evidence"
        });
        let trajectories = vec![Trajectory {
            step_id: "stop".to_string(),
            about: "incident:run".to_string(),
            mode: "read".to_string(),
            task_family: "conformance.stop".to_string(),
            allowed_tools: None,
            visible_state: json!({}),
            target_action: target,
        }];
        let predictions = BTreeMap::from([(
            "stop".to_string(),
            json!({
                "type": "stop",
                "answer_policy": "evidence_or_unknown",
                "final_refs": ["incident:run:node:a"],
                "reason": "stopped too early"
            }),
        )]);
        let args = Args {
            trajectories: TargetInput::RawTrajectories(PathBuf::from("trajectories.jsonl")),
            predictions: Some(PathBuf::from("predictions.jsonl")),
            baseline: Baseline::Deterministic,
            resolve_prepared_payloads: false,
            output: None,
            details_output: None,
            limit: None,
            offset: 0,
        };

        let summary = evaluate(&args, &trajectories, Some(&predictions))?;

        assert_eq!(summary.counts.target_stop_actions, 1);
        assert_eq!(summary.counts.stop_correct, 0);
        assert_eq!(summary.rates.stop_accuracy, 0.0);
        assert_eq!(summary.counts.exact_action_correct, 0);
        Ok(())
    }

    #[test]
    fn navigation_policy_metrics_score_cursor_window_limit_and_page_continuation()
    -> Result<(), Box<dyn Error + Send + Sync>> {
        let near_action = json!({
            "type": "tool_call",
            "tool": "kernel_near",
            "arguments": {
                "about": "about:1",
                "around": { "sequence": 7 },
                "dimensions": { "mode": "all", "scope": "current_about" },
                "include": { "evidence": true, "raw_refs": false, "relations": true },
                "limit": { "entries": 12, "tokens": 2400 },
                "budget": { "depth": 3, "tokens": 2400 },
                "window": { "before_entries": 6, "after_entries": 0 }
            }
        });
        let trace_action = json!({
            "type": "tool_call",
            "tool": "kernel_trace",
            "arguments": {
                "from": "node:1",
                "to": "node:3",
                "budget": { "depth": 2, "tokens": 2400 },
                "page": { "entries": 16, "cursor": "16" }
            }
        });
        let trajectories = vec![
            Trajectory {
                step_id: "near".to_string(),
                about: "about:1".to_string(),
                mode: "read".to_string(),
                task_family: "conformance.temporal".to_string(),
                allowed_tools: None,
                visible_state: json!({ "current_ref": "node:1" }),
                target_action: near_action.clone(),
            },
            Trajectory {
                step_id: "trace".to_string(),
                about: "about:1".to_string(),
                mode: "read".to_string(),
                task_family: "conformance.trace".to_string(),
                allowed_tools: None,
                visible_state: json!({ "current_ref": "node:1" }),
                target_action: trace_action.clone(),
            },
        ];
        let predictions = BTreeMap::from([
            ("near".to_string(), near_action),
            ("trace".to_string(), trace_action),
        ]);
        let args = Args {
            trajectories: TargetInput::RawTrajectories(PathBuf::from("trajectories.jsonl")),
            predictions: Some(PathBuf::from("predictions.jsonl")),
            baseline: Baseline::Deterministic,
            resolve_prepared_payloads: false,
            output: None,
            details_output: None,
            limit: None,
            offset: 0,
        };

        let summary = evaluate(&args, &trajectories, Some(&predictions))?;

        assert_eq!(summary.counts.target_cursor_actions, 1);
        assert_eq!(summary.counts.cursor_mode_correct, 1);
        assert_eq!(summary.counts.target_window_actions, 1);
        assert_eq!(summary.counts.window_shape_correct, 1);
        assert_eq!(summary.counts.target_limit_actions, 1);
        assert_eq!(summary.counts.limit_policy_correct, 1);
        assert_eq!(summary.counts.target_page_continuations, 1);
        assert_eq!(summary.counts.continue_page_correct, 1);
        assert_eq!(summary.rates.cursor_mode_accuracy, 1.0);
        assert_eq!(summary.rates.window_shape_accuracy, 1.0);
        assert_eq!(summary.rates.limit_policy_accuracy, 1.0);
        assert_eq!(summary.rates.continue_page_accuracy, 1.0);
        Ok(())
    }

    #[test]
    fn navigation_policy_metrics_detect_mismatched_cursor_and_page()
    -> Result<(), Box<dyn Error + Send + Sync>> {
        let target_near = json!({
            "type": "tool_call",
            "tool": "kernel_near",
            "arguments": {
                "about": "about:1",
                "around": { "sequence": 7 },
                "dimensions": { "mode": "all", "scope": "current_about" },
                "include": { "evidence": true, "raw_refs": false, "relations": true },
                "limit": { "entries": 12, "tokens": 2400 },
                "budget": { "depth": 3, "tokens": 2400 },
                "window": { "before_entries": 6, "after_entries": 0 }
            }
        });
        let predicted_near = json!({
            "type": "tool_call",
            "tool": "kernel_near",
            "arguments": {
                "about": "about:1",
                "around": { "time": "2026-05-14T00:00:00Z" },
                "dimensions": { "mode": "all", "scope": "current_about" },
                "include": { "evidence": true, "raw_refs": false, "relations": true },
                "limit": { "entries": 24, "tokens": 2400 },
                "budget": { "depth": 3, "tokens": 2400 },
                "window": { "before_entries": 3, "after_entries": 3 }
            }
        });
        let target_trace = json!({
            "type": "tool_call",
            "tool": "kernel_trace",
            "arguments": {
                "from": "node:1",
                "to": "node:3",
                "budget": { "depth": 2, "tokens": 2400 },
                "page": { "entries": 16, "cursor": "16" }
            }
        });
        let predicted_trace = json!({
            "type": "tool_call",
            "tool": "kernel_trace",
            "arguments": {
                "from": "node:1",
                "to": "node:3",
                "budget": { "depth": 2, "tokens": 2400 },
                "page": { "entries": 16, "cursor": "32" }
            }
        });
        let trajectories = vec![
            Trajectory {
                step_id: "near".to_string(),
                about: "about:1".to_string(),
                mode: "read".to_string(),
                task_family: "conformance.temporal".to_string(),
                allowed_tools: None,
                visible_state: json!({}),
                target_action: target_near,
            },
            Trajectory {
                step_id: "trace".to_string(),
                about: "about:1".to_string(),
                mode: "read".to_string(),
                task_family: "conformance.trace".to_string(),
                allowed_tools: None,
                visible_state: json!({}),
                target_action: target_trace,
            },
        ];
        let predictions = BTreeMap::from([
            ("near".to_string(), predicted_near),
            ("trace".to_string(), predicted_trace),
        ]);
        let args = Args {
            trajectories: TargetInput::RawTrajectories(PathBuf::from("trajectories.jsonl")),
            predictions: Some(PathBuf::from("predictions.jsonl")),
            baseline: Baseline::Deterministic,
            resolve_prepared_payloads: false,
            output: None,
            details_output: None,
            limit: None,
            offset: 0,
        };

        let summary = evaluate(&args, &trajectories, Some(&predictions))?;

        assert_eq!(summary.counts.tool_correct, 2);
        assert_eq!(summary.counts.cursor_mode_correct, 0);
        assert_eq!(summary.counts.window_shape_correct, 0);
        assert_eq!(summary.counts.limit_policy_correct, 0);
        assert_eq!(summary.counts.continue_page_correct, 0);
        assert_eq!(summary.rates.cursor_mode_accuracy, 0.0);
        assert_eq!(summary.rates.window_shape_accuracy, 0.0);
        assert_eq!(summary.rates.limit_policy_accuracy, 0.0);
        assert_eq!(summary.rates.continue_page_accuracy, 0.0);
        Ok(())
    }

    #[test]
    fn read_predictions_rejects_duplicate_step_ids() -> Result<(), Box<dyn Error + Send + Sync>> {
        let path = env::temp_dir().join(format!(
            "kernel-operator-policy-eval-duplicate-predictions-{}.jsonl",
            std::process::id()
        ));
        fs::write(
            &path,
            r#"{"step_id":"s1","action":{"type":"stop"}}"#.to_string()
                + "\n"
                + r#"{"step_id":"s1","action":{"type":"stop"}}"#
                + "\n",
        )?;
        let result = read_predictions(&path);
        let _ = fs::remove_file(&path);
        let error = result.expect_err("duplicate prediction step ids should fail");
        assert!(error.to_string().contains("duplicate prediction step_id"));
        Ok(())
    }

    #[test]
    fn read_trajectories_rejects_duplicate_step_ids() -> Result<(), Box<dyn Error + Send + Sync>> {
        let path = env::temp_dir().join(format!(
            "kernel-operator-policy-eval-duplicate-trajectories-{}.jsonl",
            std::process::id()
        ));
        let row = json!({
            "step_id": "s1",
            "about": "about:1",
            "mode": "read",
            "task_family": "test",
            "allowed_tools": [],
            "visible_state": {},
            "target_action": { "type": "stop" }
        });
        fs::write(&path, format!("{row}\n{row}\n"))?;
        let result = read_trajectories(&path);
        let _ = fs::remove_file(&path);
        let error = result.expect_err("duplicate trajectory step ids should fail");
        assert!(error.to_string().contains("duplicate step_id"));
        Ok(())
    }

    #[test]
    fn read_trajectories_requires_allowed_tools() -> Result<(), Box<dyn Error + Send + Sync>> {
        let path = env::temp_dir().join(format!(
            "kernel-operator-policy-eval-missing-allowed-tools-{}.jsonl",
            std::process::id()
        ));
        let row = json!({
            "step_id": "s1",
            "about": "about:1",
            "mode": "read",
            "task_family": "test",
            "visible_state": {},
            "target_action": { "type": "stop" }
        });
        fs::write(&path, format!("{row}\n"))?;
        let result = read_trajectories(&path);
        let _ = fs::remove_file(&path);
        let error = result.expect_err("raw trajectory rows must declare allowed_tools");
        assert!(
            error
                .to_string()
                .contains("missing required array field `allowed_tools`")
        );
        Ok(())
    }

    #[test]
    fn read_trajectories_rejects_allowed_tools_outside_mode()
    -> Result<(), Box<dyn Error + Send + Sync>> {
        let path = env::temp_dir().join(format!(
            "kernel-operator-policy-eval-bad-mode-tools-{}.jsonl",
            std::process::id()
        ));
        let row = json!({
            "step_id": "s1",
            "about": "about:1",
            "mode": "read",
            "task_family": "test",
            "allowed_tools": ["kernel_near", "kernel_write_memory"],
            "visible_state": {},
            "target_action": { "type": "stop" }
        });
        fs::write(&path, format!("{row}\n"))?;
        let result = read_trajectories(&path);
        let _ = fs::remove_file(&path);
        let error = result.expect_err("raw trajectory rows must keep tools inside mode");
        assert!(
            error
                .to_string()
                .contains("allowed_tools outside mode `read`: kernel_write_memory")
        );
        Ok(())
    }

    #[test]
    fn model_facing_eval_uses_assistant_action_as_target()
    -> Result<(), Box<dyn Error + Send + Sync>> {
        let path = env::temp_dir().join(format!(
            "kernel-operator-policy-eval-model-facing-{}.jsonl",
            std::process::id()
        ));
        let user = json!({
            "about": "about_0001",
            "mode": "read",
            "task_family": "conformance.read.near",
            "allowed_tools": ["kernel_near"],
            "visible_state": {
                "current_ref": "ref_0001"
            }
        });
        let action = json!({
            "type": "tool_call",
            "tool": "kernel_near",
            "arguments": {
                "about": "about_0001",
                "around": { "ref": "ref_0001" },
                "dimensions": { "mode": "all", "scope": "current_about" },
                "include": { "evidence": true, "raw_refs": false, "relations": true },
                "limit": { "entries": 8, "tokens": 1200 },
                "budget": { "depth": 3, "tokens": 2400 },
                "window": { "before_entries": 4, "after_entries": 0 }
            }
        });
        let row = json!({
            "id": "s1",
            "mode": "read",
            "task_family": "conformance.read.near",
            "messages": [
                { "role": "system", "content": "return JSON" },
                { "role": "user", "content": user.to_string() },
                { "role": "assistant", "content": json!({ "action": action }).to_string() }
            ]
        });
        fs::write(&path, format!("{row}\n"))?;

        let trajectories = read_model_facing_eval(&path)?;
        let _ = fs::remove_file(&path);

        assert_eq!(trajectories.len(), 1);
        assert_eq!(trajectories[0].step_id, "s1");
        assert_eq!(trajectories[0].about, "about_0001");
        assert_eq!(trajectories[0].target_action, action);
        Ok(())
    }

    #[test]
    fn prepared_payload_resolution_scores_final_kmp_action()
    -> Result<(), Box<dyn Error + Send + Sync>> {
        let prepared_arguments = json!({
            "about": "about_0001",
            "intent": "record_turn",
            "actor": "operator:test",
            "observed_at": "2026-05-17T00:00:00Z",
            "scope": { "process": "process_0001", "task": "about_0001" },
            "current": {
                "kind": "turn",
                "summary": "Record deterministic prepared payload execution.",
                "evidence": "The writer prepared this payload after reading context."
            },
            "connect_to": [{
                "ref": "ref_0001",
                "rel": "follows",
                "class": "procedural",
                "why": "The note follows the previous write step.",
                "evidence": "No stronger relation is visible.",
                "confidence": "medium"
            }],
            "read_context": {
                "inspected_refs": ["ref_0001"]
            },
            "idempotency_key": "writer-exec:test",
            "options": {
                "dry_run": true,
                "strict": true
            }
        });
        let trajectories = vec![Trajectory {
            step_id: "write".to_string(),
            about: "about_0001".to_string(),
            mode: "write".to_string(),
            task_family: "conformance.writer_exec.v1".to_string(),
            allowed_tools: None,
            visible_state: json!({
                "draft_write": {
                    "prepared_arguments": prepared_arguments
                }
            }),
            target_action: json!({
                "type": "prepared_tool_call",
                "tool": "kernel_write_memory",
                "source": "draft_write.prepared_arguments"
            }),
        }];
        let predictions = BTreeMap::from([(
            "write".to_string(),
            json!({
                "type": "prepared_tool_call",
                "tool": "kernel_write_memory",
                "source": "draft_write.prepared_arguments"
            }),
        )]);
        let args = Args {
            trajectories: TargetInput::ModelFacingEval(PathBuf::from("eval.jsonl")),
            predictions: Some(PathBuf::from("predictions.jsonl")),
            baseline: Baseline::Deterministic,
            resolve_prepared_payloads: true,
            output: None,
            details_output: None,
            limit: None,
            offset: 0,
        };

        let summary = evaluate(&args, &trajectories, Some(&predictions))?;

        assert!(summary.prepared_payload_resolution);
        assert_eq!(summary.counts.exact_action_correct, 1);
        assert_eq!(summary.counts.tool_correct, 1);
        assert_eq!(summary.counts.primary_refs_correct, 1);
        assert_eq!(summary.counts.invalid_predictions, 0);
        Ok(())
    }

    #[test]
    fn prepared_payload_targets_require_resolution_flag() -> Result<(), Box<dyn Error + Send + Sync>>
    {
        let trajectories = vec![Trajectory {
            step_id: "write".to_string(),
            about: "about_0001".to_string(),
            mode: "write".to_string(),
            task_family: "conformance.writer_exec.v1".to_string(),
            allowed_tools: None,
            visible_state: json!({}),
            target_action: json!({
                "type": "prepared_tool_call",
                "tool": "kernel_write_memory",
                "source": "draft_write.prepared_arguments"
            }),
        }];
        let args = Args {
            trajectories: TargetInput::ModelFacingEval(PathBuf::from("eval.jsonl")),
            predictions: None,
            baseline: Baseline::Deterministic,
            resolve_prepared_payloads: false,
            output: None,
            details_output: None,
            limit: None,
            offset: 0,
        };

        let error = evaluate_internal(&args, &trajectories, None)
            .expect_err("prepared targets must fail without resolution");

        assert_eq!(
            error.to_string(),
            "model-facing prepared payload targets require --resolve-prepared-payloads"
        );
        Ok(())
    }
}
