use std::collections::BTreeMap;
use std::env;
use std::error::Error;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::{Value, json};

use rehydration_testkit::{kernel_operator_is_bounded_tool_call, kernel_operator_primary_refs};

const EVALUATOR: &str = "kernel-operator-policy-eval-v1";

#[derive(Debug, Clone, PartialEq, Eq)]
struct Args {
    trajectories: PathBuf,
    predictions: Option<PathBuf>,
    baseline: Baseline,
    output: Option<PathBuf>,
    limit: Option<usize>,
    offset: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Baseline {
    Deterministic,
    Oracle,
}

#[derive(Debug, Clone)]
struct Trajectory {
    step_id: String,
    about: String,
    mode: String,
    task_family: String,
    visible_state: Value,
    target_action: Value,
}

#[derive(Debug, Serialize)]
struct EvalSummary {
    evaluator: &'static str,
    generated_at_unix_seconds: u64,
    trajectories: String,
    predictions: Option<String>,
    predictor: String,
    total: usize,
    by_mode: BTreeMap<String, usize>,
    by_task_family: BTreeMap<String, usize>,
    target_actions: BTreeMap<String, usize>,
    predicted_actions: BTreeMap<String, usize>,
    counts: EvalCounts,
    rates: EvalRates,
}

#[derive(Debug, Default, Serialize)]
struct EvalCounts {
    target_tool_calls: usize,
    target_stop_actions: usize,
    missing_predictions: usize,
    invalid_predictions: usize,
    unbounded_tool_calls: usize,
    action_type_correct: usize,
    tool_correct: usize,
    primary_refs_correct: usize,
    scope_correct: usize,
    stop_correct: usize,
    exact_action_correct: usize,
}

#[derive(Debug, Default, Serialize)]
struct EvalRates {
    action_type_accuracy: f64,
    tool_accuracy: f64,
    primary_ref_accuracy: f64,
    scope_accuracy: f64,
    stop_accuracy: f64,
    exact_action_accuracy: f64,
    invalid_prediction_rate: f64,
    unbounded_tool_call_rate: f64,
}

fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let args = parse_args(env::args().skip(1))?;
    let trajectories = select_trajectories(read_trajectories(&args.trajectories)?, &args);
    let predictions = match args.predictions.as_deref() {
        Some(path) => Some(read_predictions(path)?),
        None => None,
    };
    let summary = evaluate(&args, &trajectories, predictions.as_ref())?;
    let rendered = serde_json::to_string_pretty(&summary)?;
    if let Some(output) = args.output.as_deref() {
        let file = File::create(output)?;
        let mut writer = BufWriter::new(file);
        writer.write_all(rendered.as_bytes())?;
        writer.write_all(b"\n")?;
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
    let mut output = None;
    let mut limit = None;
    let mut offset = 0usize;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--trajectories" => {
                trajectories = Some(PathBuf::from(next_arg(&mut args, "--trajectories")?));
            }
            "--predictions" => {
                predictions = Some(PathBuf::from(next_arg(&mut args, "--predictions")?));
            }
            "--baseline" => {
                baseline = parse_baseline(&next_arg(&mut args, "--baseline")?)?;
            }
            "--output" => output = Some(PathBuf::from(next_arg(&mut args, "--output")?)),
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
                trajectories = Some(PathBuf::from(value));
            }
        }
    }

    Ok(Args {
        trajectories: trajectories.ok_or_else(usage)?,
        predictions,
        baseline,
        output,
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

fn next_arg(
    args: &mut impl Iterator<Item = String>,
    name: &str,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    args.next()
        .ok_or_else(|| format!("{name} requires a value").into())
}

fn usage() -> String {
    "usage: kernel_operator_policy_eval --trajectories <trajectories.jsonl> [--predictions predictions.jsonl] [--baseline deterministic|oracle] [--output summary.json] [--limit n] [--offset n]".to_string()
}

fn read_trajectories(path: &Path) -> Result<Vec<Trajectory>, Box<dyn Error + Send + Sync>> {
    read_jsonl(path)?
        .into_iter()
        .enumerate()
        .map(|(index, value)| {
            let location = format!("{}:{}", path.display(), index + 1);
            Ok(Trajectory {
                step_id: required_string(&value, "step_id", &location)?.to_string(),
                about: required_string(&value, "about", &location)?.to_string(),
                mode: required_string(&value, "mode", &location)?.to_string(),
                task_family: required_string(&value, "task_family", &location)?.to_string(),
                visible_state: value
                    .get("visible_state")
                    .cloned()
                    .ok_or_else(|| format!("{location} missing required field `visible_state`"))?,
                target_action: value
                    .get("target_action")
                    .cloned()
                    .ok_or_else(|| format!("{location} missing required field `target_action`"))?,
            })
        })
        .collect()
}

fn select_trajectories(values: Vec<Trajectory>, args: &Args) -> Vec<Trajectory> {
    values
        .into_iter()
        .skip(args.offset)
        .take(args.limit.unwrap_or(usize::MAX))
        .collect()
}

fn read_predictions(path: &Path) -> Result<BTreeMap<String, Value>, Box<dyn Error + Send + Sync>> {
    let mut predictions = BTreeMap::new();
    for (index, value) in read_jsonl(path)?.into_iter().enumerate() {
        let location = format!("{}:{}", path.display(), index + 1);
        let step_id = required_string(&value, "step_id", &location)?;
        let action = value
            .get("action")
            .or_else(|| value.get("target_action"))
            .cloned()
            .ok_or_else(|| format!("{location} missing `action` or `target_action`"))?;
        predictions.insert(step_id.to_string(), action);
    }
    Ok(predictions)
}

fn read_jsonl(path: &Path) -> Result<Vec<Value>, Box<dyn Error + Send + Sync>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut values = Vec::new();
    for (index, line) in reader.lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        values.push(serde_json::from_str(&line).map_err(|error| {
            format!(
                "failed to parse {} line {}: {error}",
                path.display(),
                index + 1
            )
        })?);
    }
    Ok(values)
}

fn required_string<'a>(
    value: &'a Value,
    field: &str,
    location: &str,
) -> Result<&'a str, Box<dyn Error + Send + Sync>> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{location} missing required string field `{field}`").into())
}

fn evaluate(
    args: &Args,
    trajectories: &[Trajectory],
    predictions: Option<&BTreeMap<String, Value>>,
) -> Result<EvalSummary, Box<dyn Error + Send + Sync>> {
    let mut counts = EvalCounts::default();
    let mut by_mode = BTreeMap::<String, usize>::new();
    let mut by_task_family = BTreeMap::<String, usize>::new();
    let mut target_actions = BTreeMap::<String, usize>::new();
    let mut predicted_actions = BTreeMap::<String, usize>::new();

    for trajectory in trajectories {
        *by_mode.entry(trajectory.mode.clone()).or_default() += 1;
        *by_task_family
            .entry(trajectory.task_family.clone())
            .or_default() += 1;
        *target_actions
            .entry(action_label(&trajectory.target_action))
            .or_default() += 1;
        match action_type(&trajectory.target_action) {
            Some("tool_call") => counts.target_tool_calls += 1,
            Some("stop") => counts.target_stop_actions += 1,
            _ => {}
        }

        let predicted = predicted_action(args.baseline, trajectory, predictions);
        let Some(predicted) = predicted else {
            counts.missing_predictions += 1;
            continue;
        };
        *predicted_actions
            .entry(action_label(&predicted))
            .or_default() += 1;
        if !is_valid_action(&predicted) {
            counts.invalid_predictions += 1;
            continue;
        }
        if is_unbounded_tool_call(&predicted) {
            counts.unbounded_tool_calls += 1;
        }
        score_action(&trajectory.target_action, &predicted, &mut counts);
    }

    let rates = rates(&counts, trajectories.len());
    Ok(EvalSummary {
        evaluator: EVALUATOR,
        generated_at_unix_seconds: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
        trajectories: args.trajectories.display().to_string(),
        predictions: args
            .predictions
            .as_ref()
            .map(|path| path.display().to_string()),
        predictor: match args.predictions.as_ref() {
            Some(_) => "predictions".to_string(),
            None => match args.baseline {
                Baseline::Deterministic => "baseline:deterministic".to_string(),
                Baseline::Oracle => "baseline:oracle".to_string(),
            },
        },
        total: trajectories.len(),
        by_mode,
        by_task_family,
        target_actions,
        predicted_actions,
        counts,
        rates,
    })
}

fn predicted_action(
    baseline: Baseline,
    trajectory: &Trajectory,
    predictions: Option<&BTreeMap<String, Value>>,
) -> Option<Value> {
    if let Some(predictions) = predictions {
        return predictions.get(&trajectory.step_id).cloned();
    }
    match baseline {
        Baseline::Oracle => Some(trajectory.target_action.clone()),
        Baseline::Deterministic => Some(deterministic_action(trajectory)),
    }
}

fn deterministic_action(trajectory: &Trajectory) -> Value {
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

fn score_action(target: &Value, predicted: &Value, counts: &mut EvalCounts) {
    let target_type = action_type(target);
    let predicted_type = action_type(predicted);
    if target_type == predicted_type {
        counts.action_type_correct += 1;
    }
    if target_type == Some("stop") && predicted_type == Some("stop") {
        counts.stop_correct += 1;
    }
    if target == predicted {
        counts.exact_action_correct += 1;
    }
    if target_type == Some("tool_call") && predicted_type == Some("tool_call") {
        if tool(target) == tool(predicted) {
            counts.tool_correct += 1;
        }
        if kernel_operator_primary_refs(target) == kernel_operator_primary_refs(predicted) {
            counts.primary_refs_correct += 1;
        }
        if scope_about(target) == scope_about(predicted) {
            counts.scope_correct += 1;
        }
    }
}

fn rates(counts: &EvalCounts, total: usize) -> EvalRates {
    EvalRates {
        action_type_accuracy: ratio(counts.action_type_correct, total),
        tool_accuracy: ratio(counts.tool_correct, counts.target_tool_calls),
        primary_ref_accuracy: ratio(counts.primary_refs_correct, counts.target_tool_calls),
        scope_accuracy: ratio(counts.scope_correct, counts.target_tool_calls),
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

fn is_valid_action(action: &Value) -> bool {
    match action_type(action) {
        Some("tool_call") => tool(action).is_some() && action.get("arguments").is_some(),
        Some("stop") => true,
        _ => false,
    }
}

fn is_unbounded_tool_call(action: &Value) -> bool {
    if action_type(action) != Some("tool_call") {
        return false;
    }
    let Some(tool) = tool(action) else {
        return true;
    };
    let arguments = action.get("arguments").unwrap_or(&Value::Null);
    !kernel_operator_is_bounded_tool_call(tool, arguments)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_policy_starts_with_near() -> Result<(), Box<dyn Error + Send + Sync>> {
        let trajectory = Trajectory {
            step_id: "s1".to_string(),
            about: "memoryarena:run:r1:task_type:progressive_search:task:1".to_string(),
            mode: "read".to_string(),
            task_family: "memoryarena.progressive_search".to_string(),
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
            visible_state: json!({
                "current_ref": "node:1",
                "last_tool": null,
            }),
            target_action: json!({
                "type": "tool_call",
                "tool": "kernel_inspect",
                "arguments": {
                    "ref": "node:1",
                    "include": { "raw": false }
                }
            }),
        }];
        let args = Args {
            trajectories: PathBuf::from("trajectories.jsonl"),
            predictions: None,
            baseline: Baseline::Oracle,
            output: None,
            limit: None,
            offset: 0,
        };
        let summary = evaluate(&args, &trajectories, None)?;
        assert_eq!(summary.counts.exact_action_correct, 1);
        assert_eq!(summary.counts.tool_correct, 1);
        Ok(())
    }

    #[test]
    fn unbounded_prediction_is_counted() -> Result<(), Box<dyn Error + Send + Sync>> {
        let trajectories = vec![Trajectory {
            step_id: "s1".to_string(),
            about: "memoryarena:run:r1:task_type:progressive_search:task:1".to_string(),
            mode: "read".to_string(),
            task_family: "memoryarena.progressive_search".to_string(),
            visible_state: json!({
                "current_ref": "node:1",
                "last_tool": null,
            }),
            target_action: json!({
                "type": "tool_call",
                "tool": "kernel_near",
                "arguments": {
                    "around": { "ref": "node:1" },
                    "limit": { "entries": 12, "tokens": 2400 }
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
                    "around": { "ref": "node:1" },
                    "limit": { "entries": 1000, "tokens": 2400 }
                }
            }),
        );
        let args = Args {
            trajectories: PathBuf::from("trajectories.jsonl"),
            predictions: Some(PathBuf::from("predictions.jsonl")),
            baseline: Baseline::Deterministic,
            output: None,
            limit: None,
            offset: 0,
        };
        let summary = evaluate(&args, &trajectories, Some(&predictions))?;
        assert_eq!(summary.counts.unbounded_tool_calls, 1);
        Ok(())
    }
}
