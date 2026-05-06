use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::error::Error;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rehydration_testkit::MemoryArenaExpected;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

type TaskSubtaskKey = (String, usize);
type ExpectedByAsk<'a> = BTreeMap<TaskSubtaskKey, &'a MemoryArenaExpected>;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Args {
    artifacts: PathBuf,
    run: PathBuf,
    output: PathBuf,
    limit_tasks: Option<usize>,
    force: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct RunAskResult {
    task_id: String,
    task_type: String,
    #[serde(default)]
    category: Option<String>,
    subtask_index: usize,
    question: String,
    #[serde(default)]
    allowed_known_at_refs: Vec<String>,
    #[serde(default)]
    observed_allowed_refs: Vec<String>,
    #[serde(default)]
    unexpected_refs: Vec<String>,
    #[serde(default)]
    missing_allowed_refs: Vec<String>,
    current_question_observed: bool,
    future_answer_leaked: bool,
    known_at_clean: bool,
    #[serde(default)]
    ask_answer: Option<String>,
    #[serde(default)]
    ask_elapsed_ms: u128,
}

#[derive(Debug, Clone, Deserialize)]
struct RunnerSummaryInput {
    #[serde(default)]
    total_events: Option<usize>,
    #[serde(default)]
    successful_events: Option<usize>,
    #[serde(default)]
    failed_events: Option<usize>,
    #[serde(default)]
    elapsed_ms: Option<u128>,
}

#[derive(Debug, Serialize)]
struct TaskScoreResult {
    task_id: String,
    task_type: String,
    category: Option<String>,
    final_subtask_index: usize,
    final_question: String,
    expected_answers: Vec<String>,
    candidate_answers: Vec<String>,
    chosen_answer: Option<String>,
    task_success: bool,
    candidate_answer_hit: bool,
    known_at_clean_subtasks: usize,
    known_at_clean_all_subtasks: bool,
    full_ref_recall_subtasks: usize,
    full_ref_recall_all_subtasks: bool,
    current_question_observed_subtasks: usize,
    future_answer_leaks: usize,
    unexpected_ref_asks: usize,
    missing_allowed_ref_asks: usize,
    final_allowed_known_at_refs: usize,
    final_observed_allowed_refs: usize,
    final_ask_answer_chars: usize,
    final_ask_elapsed_ms: u128,
}

#[derive(Debug, Serialize)]
struct ScoreSummary {
    benchmark: &'static str,
    scorecard: &'static str,
    generated_at_unix_seconds: u64,
    artifacts: String,
    run: String,
    tasks: usize,
    ask_count: usize,
    task_successes: usize,
    task_success_rate: f64,
    candidate_answer_hits: usize,
    candidate_answer_hit_rate: f64,
    known_at_clean_asks: usize,
    full_ref_recall_asks: usize,
    current_question_observed_asks: usize,
    future_answer_leaks: usize,
    unexpected_ref_asks: usize,
    missing_allowed_ref_asks: usize,
    runner_total_events: Option<usize>,
    runner_successful_events: Option<usize>,
    runner_failed_events: Option<usize>,
    runner_elapsed_ms: Option<u128>,
}

fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let args = parse_args(env::args().skip(1))?;
    ensure_output_dir(&args.output, args.force)?;

    let expected = read_expected(&args.artifacts.join("expected.jsonl"), args.limit_tasks)?;
    let run_results = read_run_results(&args.run.join("results.jsonl"), args.limit_tasks)?;
    let runner_summary = read_optional_runner_summary(&args.run.join("summary.json"))?;
    let task_results = score_tasks(&expected, &run_results)?;
    let summary = summarize_scorecard(&args, &task_results, &run_results, runner_summary)?;

    write_jsonl(
        &args.output.join("task_results.jsonl"),
        task_results.iter().map(serde_json::to_value),
    )?;
    write_jsonl(
        &args.output.join("hypotheses.jsonl"),
        task_results.iter().map(|item| {
            Ok(json!({
                "task_id": item.task_id,
                "hypothesis": item.chosen_answer.as_deref().unwrap_or("UNKNOWN"),
                "task_success": item.task_success
            }))
        }),
    )?;
    write_json_pretty(&args.output.join("score_summary.json"), &summary)?;

    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}

fn score_tasks(
    expected: &[MemoryArenaExpected],
    run_results: &[RunAskResult],
) -> Result<Vec<TaskScoreResult>, Box<dyn Error + Send + Sync>> {
    let expected_by_key = expected_by_ask_key(expected)?;
    let mut runs_by_task = BTreeMap::<String, Vec<&RunAskResult>>::new();

    for run in run_results {
        let key = (run.task_id.clone(), run.subtask_index);
        let expected = expected_by_key.get(&key).ok_or_else(|| {
            format!(
                "run result has no expected row for task {} subtask {}",
                key.0, key.1
            )
        })?;
        validate_matching_item(expected, run)?;
        runs_by_task
            .entry(run.task_id.clone())
            .or_default()
            .push(run);
    }

    for key in expected_by_key.keys() {
        if !run_results
            .iter()
            .any(|run| run.task_id == key.0 && run.subtask_index == key.1)
        {
            return Err(format!(
                "expected row has no run result for task {} subtask {}",
                key.0, key.1
            )
            .into());
        }
    }

    let mut scored = Vec::new();
    for (_task_id, mut task_runs) in runs_by_task {
        task_runs.sort_by_key(|run| run.subtask_index);
        let final_run = task_runs
            .last()
            .ok_or("internal error: task group has no run rows")?;
        let final_expected = expected_by_key
            .get(&(final_run.task_id.clone(), final_run.subtask_index))
            .ok_or_else(|| {
                format!(
                    "missing final expected row for task {} subtask {}",
                    final_run.task_id, final_run.subtask_index
                )
            })?;
        let expected_answers = answer_candidates_from_value(&final_expected.answer);
        let candidate_answers = final_run
            .ask_answer
            .as_deref()
            .map(answer_candidates_from_text)
            .unwrap_or_default();
        let chosen_answer = candidate_answers.last().cloned();
        let task_success = chosen_answer.as_ref().is_some_and(|candidate| {
            expected_answers
                .iter()
                .any(|expected| answers_match(expected, candidate))
        });
        let candidate_answer_hit = expected_answers.iter().any(|expected| {
            candidate_answers
                .iter()
                .any(|candidate| answers_match(expected, candidate))
        });
        let known_at_clean_subtasks = task_runs.iter().filter(|run| run.known_at_clean).count();
        let full_ref_recall_subtasks = task_runs
            .iter()
            .filter(|run| run.missing_allowed_refs.is_empty())
            .count();
        let current_question_observed_subtasks = task_runs
            .iter()
            .filter(|run| run.current_question_observed)
            .count();
        let future_answer_leaks = task_runs
            .iter()
            .filter(|run| run.future_answer_leaked)
            .count();
        let unexpected_ref_asks = task_runs
            .iter()
            .filter(|run| !run.unexpected_refs.is_empty())
            .count();
        let missing_allowed_ref_asks = task_runs
            .iter()
            .filter(|run| !run.missing_allowed_refs.is_empty())
            .count();

        scored.push(TaskScoreResult {
            task_id: final_run.task_id.clone(),
            task_type: final_run.task_type.clone(),
            category: final_run.category.clone(),
            final_subtask_index: final_run.subtask_index,
            final_question: final_run.question.clone(),
            expected_answers,
            candidate_answers,
            chosen_answer,
            task_success,
            candidate_answer_hit,
            known_at_clean_subtasks,
            known_at_clean_all_subtasks: known_at_clean_subtasks == task_runs.len(),
            full_ref_recall_subtasks,
            full_ref_recall_all_subtasks: full_ref_recall_subtasks == task_runs.len(),
            current_question_observed_subtasks,
            future_answer_leaks,
            unexpected_ref_asks,
            missing_allowed_ref_asks,
            final_allowed_known_at_refs: final_run.allowed_known_at_refs.len(),
            final_observed_allowed_refs: final_run.observed_allowed_refs.len(),
            final_ask_answer_chars: final_run.ask_answer.as_deref().unwrap_or_default().len(),
            final_ask_elapsed_ms: final_run.ask_elapsed_ms,
        });
    }

    Ok(scored)
}

fn answer_candidates_from_value(value: &Value) -> Vec<String> {
    match value {
        Value::String(value) => answer_candidates_from_text(value),
        Value::Array(values) => deduplicate_answers(
            values
                .iter()
                .flat_map(answer_candidates_from_value)
                .collect::<Vec<_>>(),
        ),
        Value::Object(object) => {
            for key in ["exact_answer", "exactAnswer", "answer", "target"] {
                if let Some(value) = object.get(key) {
                    let candidates = answer_candidates_from_value(value);
                    if !candidates.is_empty() {
                        return candidates;
                    }
                }
            }
            normalized_fallback_candidates(&value.to_string())
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {
            normalized_fallback_candidates(&value.to_string())
        }
    }
}

fn answer_candidates_from_text(value: &str) -> Vec<String> {
    let exact = exact_answer_candidates_from_text(value);
    if !exact.is_empty() {
        return exact;
    }
    normalized_fallback_candidates(value)
}

fn exact_answer_candidates_from_text(value: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    let mut use_next_non_empty_line = false;

    for line in value.lines() {
        let normalized_line = normalize_exact_answer_line(line);
        if use_next_non_empty_line {
            if !normalized_line.trim().is_empty() {
                push_candidate(&mut candidates, normalized_line.trim());
                use_next_non_empty_line = false;
            }
            continue;
        }

        let lower = normalized_line.to_ascii_lowercase();
        let Some(label_start) = lower.find("exact answer:") else {
            continue;
        };
        let answer_start = label_start + "exact answer:".len();
        let candidate = normalized_line[answer_start..].trim();
        if candidate.is_empty() {
            use_next_non_empty_line = true;
        } else {
            push_candidate(&mut candidates, candidate);
        }
    }

    candidates
}

fn normalize_exact_answer_line(line: &str) -> String {
    line.trim()
        .trim_start_matches(['-', '*', ' '])
        .replace(['*', '`'], "")
        .trim()
        .to_string()
}

fn normalized_fallback_candidates(value: &str) -> Vec<String> {
    let candidate = trim_candidate_answer(value);
    if candidate.is_empty() {
        Vec::new()
    } else {
        vec![candidate]
    }
}

fn push_candidate(candidates: &mut Vec<String>, value: &str) {
    let candidate = trim_candidate_answer(value);
    if candidate.is_empty() {
        return;
    }
    if !candidates.iter().any(|existing| existing == &candidate) {
        candidates.push(candidate);
    }
}

fn trim_candidate_answer(value: &str) -> String {
    let mut trimmed = value
        .trim()
        .trim_matches('"')
        .trim()
        .trim_start_matches("Answer:")
        .trim()
        .trim_end_matches('.')
        .trim()
        .to_string();
    if let Some((before_confidence, _)) = trimmed.split_once("Confidence:") {
        trimmed = before_confidence.trim().to_string();
    }
    trimmed
}

fn deduplicate_answers(values: Vec<String>) -> Vec<String> {
    let mut deduplicated = Vec::new();
    for value in values {
        if !deduplicated.iter().any(|existing| existing == &value) {
            deduplicated.push(value);
        }
    }
    deduplicated
}

fn answers_match(expected: &str, candidate: &str) -> bool {
    for expected in answer_match_alternatives(expected) {
        for candidate in answer_match_alternatives(candidate) {
            if normalized_answers_match(&expected, &candidate) {
                return true;
            }
        }
    }
    false
}

fn answer_match_alternatives(value: &str) -> Vec<String> {
    let mut alternatives = Vec::new();
    push_normalized_alternative(&mut alternatives, value);

    if let Some((before_parentheses, after_parentheses)) = value.split_once('(') {
        push_normalized_alternative(&mut alternatives, before_parentheses);
        if let Some((inside_parentheses, _)) = after_parentheses.split_once(')') {
            for alias in alias_fragments(inside_parentheses) {
                push_normalized_alternative(&mut alternatives, alias);
            }
        }
    }

    alternatives
}

fn alias_fragments(value: &str) -> Vec<&str> {
    let mut aliases = Vec::new();
    for fragment in value.split(',') {
        let fragment = fragment.trim();
        let fragment = fragment
            .strip_prefix("also written as ")
            .or_else(|| fragment.strip_prefix("also referred to as "))
            .or_else(|| fragment.strip_prefix("also known as "))
            .or_else(|| fragment.strip_prefix("aka "))
            .unwrap_or(fragment)
            .trim();
        for alias in fragment.split(" or ") {
            aliases.push(alias.trim());
        }
    }
    aliases
}

fn push_normalized_alternative(alternatives: &mut Vec<String>, value: &str) {
    let normalized = normalize_for_answer_match(value);
    if normalized.is_empty() {
        return;
    }
    if !alternatives.iter().any(|existing| existing == &normalized) {
        alternatives.push(normalized);
    }
}

fn normalized_answers_match(expected: &str, candidate: &str) -> bool {
    if expected == candidate {
        return true;
    }
    let expected_tokens = expected.split_whitespace().count();
    let candidate_tokens = candidate.split_whitespace().count();
    expected_tokens >= 2
        && candidate_tokens >= 2
        && (expected.contains(candidate) || candidate.contains(expected))
}

fn normalize_for_answer_match(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());
    let mut previous_was_space = true;
    for ch in value.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch);
            previous_was_space = false;
        } else if !previous_was_space {
            normalized.push(' ');
            previous_was_space = true;
        }
    }
    normalized.trim().to_string()
}

fn summarize_scorecard(
    args: &Args,
    task_results: &[TaskScoreResult],
    run_results: &[RunAskResult],
    runner_summary: Option<RunnerSummaryInput>,
) -> Result<ScoreSummary, Box<dyn Error + Send + Sync>> {
    let tasks = task_results.len();
    let task_successes = task_results
        .iter()
        .filter(|result| result.task_success)
        .count();
    let candidate_answer_hits = task_results
        .iter()
        .filter(|result| result.candidate_answer_hit)
        .count();
    let ask_count = run_results.len();
    let full_ref_recall_asks = run_results
        .iter()
        .filter(|result| result.missing_allowed_refs.is_empty())
        .count();
    let runner_total_events = runner_summary
        .as_ref()
        .and_then(|summary| summary.total_events);
    let runner_successful_events = runner_summary
        .as_ref()
        .and_then(|summary| summary.successful_events);
    let runner_failed_events = runner_summary
        .as_ref()
        .and_then(|summary| summary.failed_events);
    let runner_elapsed_ms = runner_summary
        .as_ref()
        .and_then(|summary| summary.elapsed_ms);

    Ok(ScoreSummary {
        benchmark: "MemoryArena",
        scorecard: "memoryarena-kmp-scorecard-exact-answer-v1",
        generated_at_unix_seconds: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
        artifacts: args.artifacts.display().to_string(),
        run: args.run.display().to_string(),
        tasks,
        ask_count,
        task_successes,
        task_success_rate: ratio(task_successes, tasks),
        candidate_answer_hits,
        candidate_answer_hit_rate: ratio(candidate_answer_hits, tasks),
        known_at_clean_asks: run_results
            .iter()
            .filter(|result| result.known_at_clean)
            .count(),
        full_ref_recall_asks,
        current_question_observed_asks: run_results
            .iter()
            .filter(|result| result.current_question_observed)
            .count(),
        future_answer_leaks: run_results
            .iter()
            .filter(|result| result.future_answer_leaked)
            .count(),
        unexpected_ref_asks: run_results
            .iter()
            .filter(|result| !result.unexpected_refs.is_empty())
            .count(),
        missing_allowed_ref_asks: run_results
            .iter()
            .filter(|result| !result.missing_allowed_refs.is_empty())
            .count(),
        runner_total_events,
        runner_successful_events,
        runner_failed_events,
        runner_elapsed_ms,
    })
}

fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

fn expected_by_ask_key(
    expected: &[MemoryArenaExpected],
) -> Result<ExpectedByAsk<'_>, Box<dyn Error + Send + Sync>> {
    let mut by_key = BTreeMap::new();
    for item in expected {
        let key = (item.task_id.clone(), item.subtask_index);
        if by_key.insert(key.clone(), item).is_some() {
            return Err(format!(
                "duplicate expected row for task {} subtask {}",
                key.0, key.1
            )
            .into());
        }
    }
    Ok(by_key)
}

fn validate_matching_item(
    expected: &MemoryArenaExpected,
    run: &RunAskResult,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    if expected.task_id != run.task_id {
        return Err(format!(
            "task_id mismatch: expected={} run={}",
            expected.task_id, run.task_id
        )
        .into());
    }
    if expected.task_type != run.task_type {
        return Err(format!(
            "task_type mismatch for {}: expected={} run={}",
            expected.task_id, expected.task_type, run.task_type
        )
        .into());
    }
    if expected.subtask_index != run.subtask_index {
        return Err(format!(
            "subtask_index mismatch for {}: expected={} run={}",
            expected.task_id, expected.subtask_index, run.subtask_index
        )
        .into());
    }
    Ok(())
}

fn read_expected(
    path: &Path,
    limit_tasks: Option<usize>,
) -> Result<Vec<MemoryArenaExpected>, Box<dyn Error + Send + Sync>> {
    let selected_task_ids = selected_task_ids(path, limit_tasks)?;
    let expected = read_jsonl(path)?
        .into_iter()
        .map(serde_json::from_value)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|expected: &MemoryArenaExpected| {
            selected_task_ids
                .as_ref()
                .is_none_or(|ids| ids.contains(&expected.task_id))
        })
        .collect::<Vec<_>>();
    Ok(expected)
}

fn read_run_results(
    path: &Path,
    limit_tasks: Option<usize>,
) -> Result<Vec<RunAskResult>, Box<dyn Error + Send + Sync>> {
    let selected_task_ids = selected_task_ids(path, limit_tasks)?;
    let run_results = read_jsonl(path)?
        .into_iter()
        .map(serde_json::from_value)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|result: &RunAskResult| {
            selected_task_ids
                .as_ref()
                .is_none_or(|ids| ids.contains(&result.task_id))
        })
        .collect::<Vec<_>>();
    Ok(run_results)
}

fn selected_task_ids(
    path: &Path,
    limit_tasks: Option<usize>,
) -> Result<Option<BTreeSet<String>>, Box<dyn Error + Send + Sync>> {
    let Some(limit) = limit_tasks else {
        return Ok(None);
    };
    let mut selected = BTreeSet::new();
    for value in read_jsonl(path)? {
        let task_id = required_string(&value, "task_id")?;
        selected.insert(task_id);
        if selected.len() >= limit {
            break;
        }
    }
    Ok(Some(selected))
}

fn read_optional_runner_summary(
    path: &Path,
) -> Result<Option<RunnerSummaryInput>, Box<dyn Error + Send + Sync>> {
    if !path.exists() {
        return Ok(None);
    }
    let file = File::open(path)?;
    Ok(Some(serde_json::from_reader(file)?))
}

fn read_jsonl(path: &Path) -> Result<Vec<Value>, Box<dyn Error + Send + Sync>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut values = Vec::new();
    for (line_index, line) in reader.lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        values.push(serde_json::from_str::<Value>(&line).map_err(|error| {
            format!(
                "invalid JSONL at {}:{}: {error}",
                path.display(),
                line_index + 1
            )
        })?);
    }
    Ok(values)
}

fn required_string(value: &Value, field: &str) -> Result<String, Box<dyn Error + Send + Sync>> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| format!("missing string field `{field}`").into())
}

fn ensure_output_dir(output: &Path, force: bool) -> Result<(), Box<dyn Error + Send + Sync>> {
    if output.exists() {
        if !output.is_dir() {
            return Err(format!("output path is not a directory: {}", output.display()).into());
        }
        if !force && output.read_dir()?.next().is_some() {
            return Err(format!(
                "output directory is not empty: {} (use --force to overwrite known artifact files)",
                output.display()
            )
            .into());
        }
    }
    fs::create_dir_all(output)?;
    Ok(())
}

fn write_jsonl<I>(path: &Path, values: I) -> Result<(), Box<dyn Error + Send + Sync>>
where
    I: Iterator<Item = Result<Value, serde_json::Error>>,
{
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    for value in values {
        serde_json::to_writer(&mut writer, &value?)?;
        writer.write_all(b"\n")?;
    }
    writer.flush()?;
    Ok(())
}

fn write_json_pretty<T: Serialize>(
    path: &Path,
    value: &T,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    serde_json::to_writer_pretty(&mut writer, value)?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}

fn parse_args(
    mut args: impl Iterator<Item = String>,
) -> Result<Args, Box<dyn Error + Send + Sync>> {
    let mut artifacts = None;
    let mut run = None;
    let mut output = None;
    let mut limit_tasks = None;
    let mut force = false;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--artifacts" => artifacts = Some(PathBuf::from(required_flag_value(&mut args, &arg)?)),
            "--run" => run = Some(PathBuf::from(required_flag_value(&mut args, &arg)?)),
            "--output" => output = Some(PathBuf::from(required_flag_value(&mut args, &arg)?)),
            "--limit-tasks" => {
                let value = required_flag_value(&mut args, &arg)?;
                let parsed = value
                    .parse::<usize>()
                    .map_err(|error| format!("invalid --limit-tasks value `{value}`: {error}"))?;
                if parsed == 0 {
                    return Err("--limit-tasks must be greater than zero".into());
                }
                limit_tasks = Some(parsed);
            }
            "--force" => force = true,
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument `{other}`").into()),
        }
    }

    Ok(Args {
        artifacts: artifacts.ok_or("--artifacts is required")?,
        run: run.ok_or("--run is required")?,
        output: output.ok_or("--output is required")?,
        limit_tasks,
        force,
    })
}

fn required_flag_value(
    args: &mut impl Iterator<Item = String>,
    flag: &str,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    args.next()
        .ok_or_else(|| format!("{flag} requires a value").into())
}

fn print_usage() {
    eprintln!(
        "Usage: memoryarena_kmp_scorecard --artifacts <adapter-output-dir> --run <runner-output-dir> --output <score-dir> [--limit-tasks N] [--force]"
    );
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn extracts_exact_answer_from_memoryarena_response() {
        let candidates = exact_answer_candidates_from_text(
            "Explanation: evidence\n\n**Exact Answer:** Ihuoma Sonia Uche\n\nConfidence: 95%",
        );

        assert_eq!(candidates, vec!["Ihuoma Sonia Uche"]);
    }

    #[test]
    fn scores_when_gold_is_one_of_candidate_answers() {
        assert!(answers_match(
            "John Daniel delos Santos (also written as Daniel Delos Santos)",
            "Daniel Delos Santos",
        ));
    }

    #[test]
    fn falls_back_to_normalized_text_when_no_exact_answer() {
        let candidates = answer_candidates_from_value(&json!("  plain answer. "));

        assert_eq!(candidates, vec!["plain answer"]);
    }
}
