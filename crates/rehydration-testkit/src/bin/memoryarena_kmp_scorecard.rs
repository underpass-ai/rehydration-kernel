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
struct SubtaskScoreResult {
    task_id: String,
    task_type: String,
    category: Option<String>,
    subtask_index: usize,
    question: String,
    expected_answer: Value,
    expected_answer_kind: String,
    expected_answers: Vec<String>,
    candidate_answers: Vec<String>,
    chosen_answer: Option<String>,
    hard_success: bool,
    candidate_answer_hit: bool,
    soft_score: Option<f64>,
    soft_score_basis: Option<&'static str>,
    known_at_clean: bool,
    full_ref_recall: bool,
    current_question_observed: bool,
    future_answer_leaked: bool,
    unexpected_refs: Vec<String>,
    missing_allowed_refs: Vec<String>,
    allowed_known_at_refs: usize,
    observed_allowed_refs: usize,
    ask_answer_chars: usize,
    ask_elapsed_ms: u128,
}

#[derive(Debug, Serialize)]
struct TaskScoreResult {
    task_id: String,
    task_type: String,
    category: Option<String>,
    subtasks: usize,
    passed_subtasks: usize,
    process_score: f64,
    soft_process_score: Option<f64>,
    task_success_rule: &'static str,
    final_subtask_index: usize,
    final_question: String,
    expected_answers: Vec<String>,
    candidate_answers: Vec<String>,
    chosen_answer: Option<String>,
    final_subtask_success: bool,
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
struct TaskTypeScoreSummary {
    tasks: usize,
    subtasks: usize,
    task_successes: usize,
    task_success_rate: f64,
    passed_subtasks: usize,
    process_score: f64,
    micro_process_score: f64,
    soft_process_score: Option<f64>,
}

#[derive(Debug, Serialize)]
struct DepthSuccessRate {
    subtasks: usize,
    successes: usize,
    success_rate: f64,
}

#[derive(Debug, Serialize)]
struct ScoreSummary {
    benchmark: &'static str,
    scorecard: &'static str,
    schema_version: &'static str,
    evaluation_protocol: &'static str,
    evaluator_limitations: &'static str,
    generated_at_unix_seconds: u64,
    artifacts: String,
    run: String,
    tasks: usize,
    subtasks: usize,
    ask_count: usize,
    task_successes: usize,
    task_success_rate: f64,
    passed_subtasks: usize,
    process_score: f64,
    micro_process_score: f64,
    soft_process_score: Option<f64>,
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
    by_task_type: BTreeMap<String, TaskTypeScoreSummary>,
    sr_at_depth: BTreeMap<usize, DepthSuccessRate>,
}

fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let args = parse_args(env::args().skip(1))?;
    ensure_output_dir(&args.output, args.force)?;

    let expected = read_expected(&args.artifacts.join("expected.jsonl"), args.limit_tasks)?;
    let run_results = read_run_results(&args.run.join("results.jsonl"), args.limit_tasks)?;
    let runner_summary = read_optional_runner_summary(&args.run.join("summary.json"))?;
    let subtask_results = score_subtasks(&expected, &run_results)?;
    let task_results = score_tasks(&subtask_results)?;
    let summary = summarize_scorecard(
        &args,
        &task_results,
        &subtask_results,
        &run_results,
        runner_summary,
    )?;

    write_jsonl(
        &args.output.join("subtask_results.jsonl"),
        subtask_results.iter().map(serde_json::to_value),
    )?;
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

fn score_subtasks(
    expected: &[MemoryArenaExpected],
    run_results: &[RunAskResult],
) -> Result<Vec<SubtaskScoreResult>, Box<dyn Error + Send + Sync>> {
    let expected_by_key = expected_by_ask_key(expected)?;
    let mut run_by_key = BTreeMap::<TaskSubtaskKey, &RunAskResult>::new();

    for run in run_results {
        let key = (run.task_id.clone(), run.subtask_index);
        let expected = expected_by_key.get(&key).ok_or_else(|| {
            format!(
                "run result has no expected row for task {} subtask {}",
                key.0, key.1
            )
        })?;
        validate_matching_item(expected, run)?;
        if run_by_key.insert(key.clone(), run).is_some() {
            return Err(
                format!("duplicate run result for task {} subtask {}", key.0, key.1).into(),
            );
        }
    }

    for key in expected_by_key.keys() {
        if !run_by_key.contains_key(key) {
            return Err(format!(
                "expected row has no run result for task {} subtask {}",
                key.0, key.1
            )
            .into());
        }
    }

    let mut scored = Vec::new();
    for (key, expected) in expected_by_key {
        let run = run_by_key
            .get(&key)
            .ok_or_else(|| format!("missing run result for task {} subtask {}", key.0, key.1))?;
        let candidate_answers = run
            .ask_answer
            .as_deref()
            .map(answer_candidates_from_text)
            .unwrap_or_default();
        let chosen_answer = candidate_answers.last().cloned();
        let answer_score = score_answer(
            &expected.task_type,
            &expected.answer,
            run.ask_answer.as_deref(),
        );

        scored.push(SubtaskScoreResult {
            task_id: run.task_id.clone(),
            task_type: run.task_type.clone(),
            category: run.category.clone(),
            subtask_index: run.subtask_index,
            question: run.question.clone(),
            expected_answer: expected.answer.clone(),
            expected_answer_kind: answer_score.expected_answer_kind,
            expected_answers: answer_score.expected_answers,
            candidate_answers,
            chosen_answer,
            hard_success: answer_score.hard_success,
            candidate_answer_hit: answer_score.candidate_answer_hit,
            soft_score: answer_score.soft_score,
            soft_score_basis: answer_score.soft_score_basis,
            known_at_clean: run.known_at_clean,
            full_ref_recall: run.missing_allowed_refs.is_empty(),
            current_question_observed: run.current_question_observed,
            future_answer_leaked: run.future_answer_leaked,
            unexpected_refs: run.unexpected_refs.clone(),
            missing_allowed_refs: run.missing_allowed_refs.clone(),
            allowed_known_at_refs: run.allowed_known_at_refs.len(),
            observed_allowed_refs: run.observed_allowed_refs.len(),
            ask_answer_chars: run.ask_answer.as_deref().unwrap_or_default().len(),
            ask_elapsed_ms: run.ask_elapsed_ms,
        });
    }

    scored.sort_by(|left, right| {
        left.task_id
            .cmp(&right.task_id)
            .then(left.subtask_index.cmp(&right.subtask_index))
    });
    Ok(scored)
}

fn score_tasks(
    subtask_results: &[SubtaskScoreResult],
) -> Result<Vec<TaskScoreResult>, Box<dyn Error + Send + Sync>> {
    let mut subtasks_by_task = BTreeMap::<String, Vec<&SubtaskScoreResult>>::new();
    for subtask in subtask_results {
        subtasks_by_task
            .entry(subtask.task_id.clone())
            .or_default()
            .push(subtask);
    }

    let mut scored = Vec::new();
    for (_task_id, mut task_subtasks) in subtasks_by_task {
        task_subtasks.sort_by_key(|subtask| subtask.subtask_index);
        let final_subtask = task_subtasks
            .last()
            .ok_or("internal error: task group has no subtask rows")?;
        let subtasks = task_subtasks.len();
        let passed_subtasks = task_subtasks
            .iter()
            .filter(|subtask| subtask.hard_success)
            .count();
        let process_score = ratio(passed_subtasks, subtasks);
        let soft_scores = task_subtasks
            .iter()
            .filter_map(|subtask| subtask.soft_score)
            .collect::<Vec<_>>();
        let soft_process_score = if soft_scores.is_empty() {
            None
        } else {
            Some(soft_scores.iter().sum::<f64>() / soft_scores.len() as f64)
        };
        let final_subtask_success = final_subtask.hard_success;
        let task_success_rule = task_success_rule(&final_subtask.task_type);
        let task_success = match task_success_rule {
            "all_subtasks_hard_success" => passed_subtasks == subtasks,
            "final_subtask_hard_success" => final_subtask_success,
            _ => final_subtask_success,
        };
        let known_at_clean_subtasks = task_subtasks
            .iter()
            .filter(|subtask| subtask.known_at_clean)
            .count();
        let full_ref_recall_subtasks = task_subtasks
            .iter()
            .filter(|subtask| subtask.full_ref_recall)
            .count();
        let current_question_observed_subtasks = task_subtasks
            .iter()
            .filter(|subtask| subtask.current_question_observed)
            .count();
        let future_answer_leaks = task_subtasks
            .iter()
            .filter(|subtask| subtask.future_answer_leaked)
            .count();
        let unexpected_ref_asks = task_subtasks
            .iter()
            .filter(|subtask| !subtask.unexpected_refs.is_empty())
            .count();
        let missing_allowed_ref_asks = task_subtasks
            .iter()
            .filter(|subtask| !subtask.missing_allowed_refs.is_empty())
            .count();

        scored.push(TaskScoreResult {
            task_id: final_subtask.task_id.clone(),
            task_type: final_subtask.task_type.clone(),
            category: final_subtask.category.clone(),
            subtasks,
            passed_subtasks,
            process_score,
            soft_process_score,
            task_success_rule,
            final_subtask_index: final_subtask.subtask_index,
            final_question: final_subtask.question.clone(),
            expected_answers: final_subtask.expected_answers.clone(),
            candidate_answers: final_subtask.candidate_answers.clone(),
            chosen_answer: final_subtask.chosen_answer.clone(),
            final_subtask_success,
            task_success,
            candidate_answer_hit: final_subtask.candidate_answer_hit,
            known_at_clean_subtasks,
            known_at_clean_all_subtasks: known_at_clean_subtasks == subtasks,
            full_ref_recall_subtasks,
            full_ref_recall_all_subtasks: full_ref_recall_subtasks == subtasks,
            current_question_observed_subtasks,
            future_answer_leaks,
            unexpected_ref_asks,
            missing_allowed_ref_asks,
            final_allowed_known_at_refs: final_subtask.allowed_known_at_refs,
            final_observed_allowed_refs: final_subtask.observed_allowed_refs,
            final_ask_answer_chars: final_subtask.ask_answer_chars,
            final_ask_elapsed_ms: final_subtask.ask_elapsed_ms,
        });
    }

    Ok(scored)
}

fn task_success_rule(task_type: &str) -> &'static str {
    match task_type {
        "bundled_shopping" | "group_travel_planner" => "all_subtasks_hard_success",
        "progressive_search" | "formal_reasoning_math" | "formal_reasoning_phys" => {
            "final_subtask_hard_success"
        }
        _ => "final_subtask_hard_success",
    }
}

#[derive(Debug)]
struct AnswerScore {
    expected_answer_kind: String,
    expected_answers: Vec<String>,
    hard_success: bool,
    candidate_answer_hit: bool,
    soft_score: Option<f64>,
    soft_score_basis: Option<&'static str>,
}

fn score_answer(task_type: &str, expected: &Value, candidate_text: Option<&str>) -> AnswerScore {
    let candidate_text = candidate_text.unwrap_or_default();
    match expected {
        Value::Object(object) if object.contains_key("target_asin") => {
            score_shopping_answer(expected, candidate_text)
        }
        Value::Array(_) if task_type == "group_travel_planner" => {
            score_travel_plan_answer(expected, candidate_text)
        }
        Value::Object(_) if task_type == "group_travel_planner" => {
            score_travel_plan_answer(expected, candidate_text)
        }
        _ => score_exact_answer(expected, candidate_text),
    }
}

fn score_shopping_answer(expected: &Value, candidate_text: &str) -> AnswerScore {
    let target_asin = expected
        .get("target_asin")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    let mut expected_answers = Vec::new();
    push_candidate(&mut expected_answers, target_asin);
    if let Some(attributes) = expected.get("attributes") {
        for attribute in answer_candidates_from_value(attributes) {
            push_candidate(&mut expected_answers, &attribute);
        }
    }
    let hard_success = !target_asin.is_empty() && text_contains_answer(candidate_text, target_asin);
    let attribute_values = expected
        .get("attributes")
        .map(meaningful_json_leaf_values)
        .unwrap_or_default();
    let soft_score = if attribute_values.is_empty() {
        None
    } else {
        let matched = attribute_values
            .iter()
            .filter(|attribute| text_contains_answer(candidate_text, attribute))
            .count();
        Some(matched as f64 / attribute_values.len() as f64)
    };

    AnswerScore {
        expected_answer_kind: "target_asin".to_string(),
        expected_answers,
        hard_success,
        candidate_answer_hit: hard_success,
        soft_score,
        soft_score_basis: soft_score.map(|_| "shopping_attribute_text_coverage"),
    }
}

fn score_travel_plan_answer(expected: &Value, candidate_text: &str) -> AnswerScore {
    let expected_answers = meaningful_json_leaf_values(expected);
    let matched = expected_answers
        .iter()
        .filter(|expected| text_contains_answer(candidate_text, expected))
        .count();
    let soft_score = if expected_answers.is_empty() {
        None
    } else {
        Some(matched as f64 / expected_answers.len() as f64)
    };
    let hard_success = !expected_answers.is_empty() && matched == expected_answers.len();

    AnswerScore {
        expected_answer_kind: "travel_plan_slots".to_string(),
        expected_answers,
        hard_success,
        candidate_answer_hit: hard_success,
        soft_score,
        soft_score_basis: soft_score.map(|_| "travel_expected_slot_text_coverage_proxy"),
    }
}

fn score_exact_answer(expected: &Value, candidate_text: &str) -> AnswerScore {
    let expected_answers = answer_candidates_from_value(expected);
    let hard_success = expected_answers
        .iter()
        .any(|expected| text_contains_answer(candidate_text, expected));

    AnswerScore {
        expected_answer_kind: expected_answer_kind(expected).to_string(),
        expected_answers,
        hard_success,
        candidate_answer_hit: hard_success,
        soft_score: None,
        soft_score_basis: None,
    }
}

fn expected_answer_kind(value: &Value) -> &'static str {
    match value {
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
    }
}

fn meaningful_json_leaf_values(value: &Value) -> Vec<String> {
    let mut values = Vec::new();
    collect_meaningful_json_leaf_values(value, &mut values);
    deduplicate_answers(values)
}

fn collect_meaningful_json_leaf_values(value: &Value, values: &mut Vec<String>) {
    match value {
        Value::String(text) => {
            let text = trim_candidate_answer(text);
            if is_meaningful_leaf_value(&text) {
                values.push(text);
            }
        }
        Value::Number(number) => values.push(number.to_string()),
        Value::Array(items) => {
            for item in items {
                collect_meaningful_json_leaf_values(item, values);
            }
        }
        Value::Object(object) => {
            for value in object.values() {
                collect_meaningful_json_leaf_values(value, values);
            }
        }
        Value::Null | Value::Bool(_) => {}
    }
}

fn is_meaningful_leaf_value(value: &str) -> bool {
    let normalized = value.trim();
    !normalized.is_empty() && normalized != "-" && normalized != "N/A"
}

fn text_contains_answer(text: &str, expected: &str) -> bool {
    if expected.trim().is_empty() {
        return false;
    }
    if answers_match(expected, text) {
        return true;
    }
    let expected = normalize_for_answer_match(expected);
    let candidate = normalize_for_answer_match(text);
    looks_like_identifier(&expected) && candidate.contains(&expected)
}

fn looks_like_identifier(value: &str) -> bool {
    let compact = value.replace(' ', "");
    compact.len() >= 8
        && compact
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
        && compact.chars().any(|ch| ch.is_ascii_digit())
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
            if let Some(value) = object.get("target_asin") {
                let candidates = answer_candidates_from_value(value);
                if !candidates.is_empty() {
                    return candidates;
                }
            }
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
    subtask_results: &[SubtaskScoreResult],
    run_results: &[RunAskResult],
    runner_summary: Option<RunnerSummaryInput>,
) -> Result<ScoreSummary, Box<dyn Error + Send + Sync>> {
    let tasks = task_results.len();
    let subtasks = subtask_results.len();
    let task_successes = task_results
        .iter()
        .filter(|result| result.task_success)
        .count();
    let passed_subtasks = subtask_results
        .iter()
        .filter(|result| result.hard_success)
        .count();
    let process_score = mean_task_process_score(task_results);
    let soft_process_score = mean_optional_task_process_score(task_results);
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
        scorecard: "memoryarena-kmp-scorecard-paper-aligned-v1",
        schema_version: "memoryarena-score-summary-v1",
        evaluation_protocol: "paper-aligned local evaluator for arXiv:2602.16313 Section 4.2 metrics: SR, PS, and SR@depth; group-travel sPS is a local slot-coverage proxy because the official environment evaluator is not published.",
        evaluator_limitations: "This is not an official MemoryArena score. It scores KMP runner answer artifacts, not a full web/travel/formal task-agent environment rollout.",
        generated_at_unix_seconds: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
        artifacts: args.artifacts.display().to_string(),
        run: args.run.display().to_string(),
        tasks,
        subtasks,
        ask_count,
        task_successes,
        task_success_rate: ratio(task_successes, tasks),
        passed_subtasks,
        process_score,
        micro_process_score: ratio(passed_subtasks, subtasks),
        soft_process_score,
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
        by_task_type: summarize_by_task_type(task_results, subtask_results),
        sr_at_depth: summarize_sr_at_depth(subtask_results),
    })
}

fn mean_task_process_score(task_results: &[TaskScoreResult]) -> f64 {
    if task_results.is_empty() {
        return 0.0;
    }
    task_results
        .iter()
        .map(|task| task.process_score)
        .sum::<f64>()
        / task_results.len() as f64
}

fn mean_optional_task_process_score(task_results: &[TaskScoreResult]) -> Option<f64> {
    let scores = task_results
        .iter()
        .filter_map(|task| task.soft_process_score)
        .collect::<Vec<_>>();
    if scores.is_empty() {
        None
    } else {
        Some(scores.iter().sum::<f64>() / scores.len() as f64)
    }
}

fn summarize_by_task_type(
    task_results: &[TaskScoreResult],
    subtask_results: &[SubtaskScoreResult],
) -> BTreeMap<String, TaskTypeScoreSummary> {
    let mut task_results_by_type = BTreeMap::<String, Vec<&TaskScoreResult>>::new();
    for task in task_results {
        task_results_by_type
            .entry(task.task_type.clone())
            .or_default()
            .push(task);
    }
    let mut subtask_results_by_type = BTreeMap::<String, Vec<&SubtaskScoreResult>>::new();
    for subtask in subtask_results {
        subtask_results_by_type
            .entry(subtask.task_type.clone())
            .or_default()
            .push(subtask);
    }

    let mut summaries = BTreeMap::new();
    for (task_type, tasks_for_type) in task_results_by_type {
        let subtasks_for_type = subtask_results_by_type
            .get(&task_type)
            .map(Vec::as_slice)
            .unwrap_or_default();
        let tasks = tasks_for_type.len();
        let subtasks = subtasks_for_type.len();
        let task_successes = tasks_for_type
            .iter()
            .filter(|task| task.task_success)
            .count();
        let passed_subtasks = subtasks_for_type
            .iter()
            .filter(|subtask| subtask.hard_success)
            .count();
        let process_score = if tasks_for_type.is_empty() {
            0.0
        } else {
            tasks_for_type
                .iter()
                .map(|task| task.process_score)
                .sum::<f64>()
                / tasks_for_type.len() as f64
        };
        let soft_scores = tasks_for_type
            .iter()
            .filter_map(|task| task.soft_process_score)
            .collect::<Vec<_>>();
        let soft_process_score = if soft_scores.is_empty() {
            None
        } else {
            Some(soft_scores.iter().sum::<f64>() / soft_scores.len() as f64)
        };
        summaries.insert(
            task_type,
            TaskTypeScoreSummary {
                tasks,
                subtasks,
                task_successes,
                task_success_rate: ratio(task_successes, tasks),
                passed_subtasks,
                process_score,
                micro_process_score: ratio(passed_subtasks, subtasks),
                soft_process_score,
            },
        );
    }
    summaries
}

fn summarize_sr_at_depth(
    subtask_results: &[SubtaskScoreResult],
) -> BTreeMap<usize, DepthSuccessRate> {
    let mut by_depth = BTreeMap::<usize, Vec<&SubtaskScoreResult>>::new();
    for subtask in subtask_results {
        by_depth
            .entry(subtask.subtask_index)
            .or_default()
            .push(subtask);
    }

    by_depth
        .into_iter()
        .map(|(depth, subtasks)| {
            let successes = subtasks
                .iter()
                .filter(|subtask| subtask.hard_success)
                .count();
            (
                depth,
                DepthSuccessRate {
                    subtasks: subtasks.len(),
                    successes,
                    success_rate: ratio(successes, subtasks.len()),
                },
            )
        })
        .collect()
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

    #[test]
    fn scores_shopping_target_asin_inside_answer_text() {
        let score = score_answer(
            "bundled_shopping",
            &json!({
                "target_asin": "B00TUDFEW2",
                "attributes": ["Almond Flour", "Vanilla Cake Mix"]
            }),
            Some("The selected item is ASIN B00TUDFEW2 because it is an almond flour cake mix."),
        );

        assert!(score.hard_success);
        assert_eq!(score.expected_answer_kind, "target_asin");
        assert_eq!(score.soft_score, Some(0.5));
    }

    #[test]
    fn task_success_rule_matches_paper_domains() {
        assert_eq!(
            task_success_rule("bundled_shopping"),
            "all_subtasks_hard_success"
        );
        assert_eq!(
            task_success_rule("progressive_search"),
            "final_subtask_hard_success"
        );
        assert_eq!(
            task_success_rule("formal_reasoning_math"),
            "final_subtask_hard_success"
        );
        assert_eq!(
            task_success_rule("formal_reasoning_phys"),
            "final_subtask_hard_success"
        );
        assert_eq!(
            task_success_rule("group_travel_planner"),
            "all_subtasks_hard_success"
        );
    }

    #[test]
    fn scores_formal_reasoning_exact_answers() {
        let math_score = score_answer(
            "formal_reasoning_math",
            &json!("The correct answer is A,B,C"),
            Some("After applying the theorem, the correct answer is A,B,C."),
        );
        let phys_score = score_answer(
            "formal_reasoning_phys",
            &json!("All vector fields on Spec(A) have flow"),
            Some("Exact Answer: All vector fields on Spec(A) have flow"),
        );

        assert!(math_score.hard_success);
        assert!(phys_score.hard_success);
        assert_eq!(math_score.expected_answer_kind, "string");
        assert_eq!(phys_score.expected_answer_kind, "string");
    }

    #[test]
    fn task_success_uses_final_subtask_for_progressive_but_all_subtasks_for_shopping() {
        let progressive = score_tasks(&[
            subtask_fixture("progressive_search", "p1", 1, false),
            subtask_fixture("progressive_search", "p1", 2, true),
        ])
        .expect("progressive task should score");
        let shopping = score_tasks(&[
            subtask_fixture("bundled_shopping", "s1", 1, true),
            subtask_fixture("bundled_shopping", "s1", 2, false),
        ])
        .expect("shopping task should score");

        assert!(progressive[0].task_success);
        assert_eq!(progressive[0].process_score, 0.5);
        assert!(!shopping[0].task_success);
        assert_eq!(shopping[0].process_score, 0.5);
    }

    #[test]
    fn computes_travel_slot_coverage_proxy() {
        let score = score_answer(
            "group_travel_planner",
            &json!([
                {
                    "days": 1,
                    "current_city": "Rockford",
                    "transportation": "Flight Number: F3573659",
                    "dinner": "Coco Bambu, Rockford",
                    "accommodation": "-"
                }
            ]),
            Some("Day 1 current city Rockford. Dinner: Coco Bambu, Rockford."),
        );

        assert!(score.soft_score.is_some_and(|score| score > 0.0));
        assert!(!score.hard_success);
        assert_eq!(
            score.soft_score_basis,
            Some("travel_expected_slot_text_coverage_proxy")
        );
    }

    fn subtask_fixture(
        task_type: &str,
        task_id: &str,
        subtask_index: usize,
        hard_success: bool,
    ) -> SubtaskScoreResult {
        SubtaskScoreResult {
            task_id: task_id.to_string(),
            task_type: task_type.to_string(),
            category: None,
            subtask_index,
            question: format!("question {subtask_index}"),
            expected_answer: json!("answer"),
            expected_answer_kind: "string".to_string(),
            expected_answers: vec!["answer".to_string()],
            candidate_answers: vec!["answer".to_string()],
            chosen_answer: Some("answer".to_string()),
            hard_success,
            candidate_answer_hit: hard_success,
            soft_score: None,
            soft_score_basis: None,
            known_at_clean: true,
            full_ref_recall: true,
            current_question_observed: true,
            future_answer_leaked: false,
            unexpected_refs: Vec::new(),
            missing_allowed_refs: Vec::new(),
            allowed_known_at_refs: subtask_index,
            observed_allowed_refs: subtask_index,
            ask_answer_chars: 6,
            ask_elapsed_ms: 1,
        }
    }
}
