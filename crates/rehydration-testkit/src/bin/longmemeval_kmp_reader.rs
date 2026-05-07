use std::collections::BTreeMap;
use std::env;
use std::error::Error;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rehydration_testkit::{LlmProvider, LongMemEvalExpected, call_llm};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone)]
struct Args {
    artifacts: PathBuf,
    run: PathBuf,
    output: PathBuf,
    endpoint: Option<String>,
    model: Option<String>,
    provider: Option<LlmProvider>,
    api_key_env: String,
    max_tokens: u32,
    temperature: f64,
    limit: Option<usize>,
    force: bool,
}

#[derive(Debug, Deserialize)]
struct RunItem {
    question_id: String,
    question_type: String,
    evidence_hit: String,
    ask_answer: Option<String>,
    ask_content: Option<Value>,
}

#[derive(Debug, Clone)]
struct EvidenceSnippet {
    ref_id: String,
    text: String,
    source: Option<String>,
    time: Option<String>,
    sequence: Option<u64>,
    why: Option<String>,
    confidence: Option<String>,
}

#[derive(Debug, Serialize)]
struct ReaderItemResult {
    question_id: String,
    question_type: String,
    question: String,
    evidence_hit: String,
    evidence_chars: usize,
    expected_answer: Value,
    hypothesis: String,
    lexical_answer_hit: bool,
    prompt_tokens: u32,
    completion_tokens: u32,
    reader_latency_ms: u128,
}

#[derive(Debug, Serialize)]
struct ReaderSummary {
    benchmark: &'static str,
    reader: &'static str,
    generated_at_unix_seconds: u64,
    artifacts: String,
    run: String,
    endpoint: String,
    model: String,
    provider: &'static str,
    total_items: usize,
    lexical_answer_items: usize,
    lexical_answer_hits: usize,
    prompt_tokens: u32,
    completion_tokens: u32,
    elapsed_ms: u128,
    question_types: BTreeMap<String, ReaderQuestionTypeSummary>,
}

#[derive(Debug, Default, Serialize)]
struct ReaderQuestionTypeSummary {
    count: usize,
    lexical_answer_hits: usize,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let args = parse_args(env::args().skip(1))?;
    let endpoint = resolve_required(args.endpoint.as_deref(), "LLM_ENDPOINT", "--endpoint")?;
    let model = resolve_required(args.model.as_deref(), "LLM_MODEL", "--model")?;
    let provider = match args.provider {
        Some(provider) => provider,
        None => match env::var("LLM_PROVIDER").ok() {
            Some(value) if !value.trim().is_empty() => parse_provider(&value)?,
            _ => detect_provider_from_model(&model),
        },
    };
    let api_key = env::var(&args.api_key_env)
        .ok()
        .filter(|value| !value.trim().is_empty());
    ensure_output_dir(&args.output, args.force)?;

    let expected = read_expected(&args.artifacts.join("expected.jsonl"), args.limit)?;
    let run_items = read_run_items(&args.run.join("results.jsonl"), args.limit)?;
    if expected.len() != run_items.len() {
        return Err(format!(
            "artifact count mismatch: expected={} run={}",
            expected.len(),
            run_items.len()
        )
        .into());
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(180))
        .build()?;
    let started = Instant::now();
    let mut reader_results = Vec::new();

    for (expected, run_item) in expected.iter().zip(run_items.iter()) {
        validate_matching_item(expected, run_item)?;
        let evidence = recovered_evidence_text(run_item)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| format!("{} has no recovered evidence to read", expected.question_id))?;
        let prompt = build_reader_prompt(
            &expected.question_type,
            &expected.question,
            expected.abstention,
            &evidence,
        );

        let reader_started = Instant::now();
        let (raw_hypothesis, prompt_tokens, completion_tokens) = call_llm(
            &client,
            &endpoint,
            &model,
            provider,
            api_key.as_deref(),
            &prompt,
            args.max_tokens,
            args.temperature,
        )
        .await?;
        let reader_latency_ms = reader_started.elapsed().as_millis();
        let hypothesis = normalize_reader_output(&raw_hypothesis);
        let lexical_answer_hit = answer_contains_expected(&expected.answer, Some(&hypothesis));

        reader_results.push(ReaderItemResult {
            question_id: expected.question_id.clone(),
            question_type: expected.question_type.clone(),
            question: expected.question.clone(),
            evidence_hit: run_item.evidence_hit.clone(),
            evidence_chars: evidence.len(),
            expected_answer: expected.answer.clone(),
            hypothesis,
            lexical_answer_hit,
            prompt_tokens,
            completion_tokens,
            reader_latency_ms,
        });
    }

    write_jsonl(
        &args.output.join("reader_results.jsonl"),
        reader_results.iter().map(serde_json::to_value),
    )?;
    write_jsonl(
        &args.output.join("hypotheses.jsonl"),
        reader_results.iter().map(|item| {
            Ok(json!({
                "question_id": item.question_id,
                "hypothesis": item.hypothesis
            }))
        }),
    )?;
    let summary = summarize_reader_run(
        &args,
        endpoint,
        model,
        provider,
        started.elapsed().as_millis(),
        &reader_results,
    )?;
    write_json_pretty(&args.output.join("summary.json"), &summary)?;

    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}

fn build_reader_prompt(
    question_type: &str,
    question: &str,
    abstention: bool,
    evidence: &str,
) -> String {
    let task_instruction = if abstention {
        "The question is unanswerable if the recovered evidence does not contain the requested fact. Answer UNKNOWN when it is not answerable."
    } else {
        match question_type {
            "multi-session" => {
                "Aggregate all relevant evidence across sessions. Count, sum, list, or combine facts when the question requires it."
            }
            "temporal-reasoning" => {
                "Reason over dates, durations, ordering, recency, and before/after relations. Compute the requested temporal answer."
            }
            "knowledge-update" => {
                "Resolve updates explicitly. Evidence is ordered from older to newer when sequence is available. Compare older and newer statements, ignore superseded values, and answer with the latest applicable value. Do not answer UNKNOWN when the latest value is explicit."
            }
            "single-session-preference" => {
                "Answer the user's request as a personalized assistant response, not as a meta-summary of preferences. Use the recovered evidence to tailor the recommendation or advice. Cover every distinct user-specific preference, constraint, setup detail, material, tool, ingredient, language, skill, venue type, activity format, or maintenance concern that would change the answer. If the request is in a new context and the evidence gives preferences but not current location-specific options, do not return UNKNOWN; answer with preference-constrained criteria or advice using only those recovered preferences. Include the user's positive preferences and avoid the options the evidence implies they would not prefer. Keep it concise but actionable."
            }
            "single-session-user" | "single-session-assistant" => {
                "Answer the factual question directly from the recovered evidence."
            }
            _ => "Answer directly from the recovered evidence.",
        }
    };

    format!(
        "You are a LongMemEval reader. Use only the recovered kernel evidence.\n\
         Do not use outside knowledge. Do not mention that evidence was provided.\n\
         Return only the final answer, with no explanation, no markdown, and no preamble.\n\
         If the evidence is insufficient, return exactly UNKNOWN.\n\n\
         Question type: {question_type}\n\
         Task: {task_instruction}\n\n\
         Question:\n{question}\n\n\
         Recovered kernel evidence:\n{evidence}\n"
    )
}

fn recovered_evidence_text(run_item: &RunItem) -> Option<String> {
    run_item
        .ask_content
        .as_ref()
        .and_then(format_structured_evidence)
        .or_else(|| run_item.ask_answer.clone())
}

fn format_structured_evidence(value: &Value) -> Option<String> {
    let mut snippets = structured_evidence_snippets(value);
    if snippets.is_empty() {
        return None;
    }
    snippets.sort_by(|left, right| {
        left.sequence
            .unwrap_or(u64::MAX)
            .cmp(&right.sequence.unwrap_or(u64::MAX))
            .then_with(|| left.ref_id.cmp(&right.ref_id))
    });

    let mut rendered = String::from(
        "Evidence is ordered by kernel relation sequence when available; later sequence means newer evidence within this adapted item.\n",
    );
    for (index, snippet) in snippets.iter().enumerate() {
        rendered.push_str(&format!("\n[{}] ref={}", index + 1, snippet.ref_id));
        if let Some(sequence) = snippet.sequence {
            rendered.push_str(&format!(" sequence={sequence}"));
        }
        if let Some(time) = snippet
            .time
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            rendered.push_str(&format!(" time={time}"));
        }
        if let Some(confidence) = snippet
            .confidence
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            rendered.push_str(&format!(" confidence={confidence}"));
        }
        rendered.push('\n');
        if let Some(source) = snippet
            .source
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            rendered.push_str(&format!("source: {source}\n"));
        }
        if let Some(why) = snippet
            .why
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            rendered.push_str(&format!("why: {why}\n"));
        }
        rendered.push_str("text: ");
        rendered.push_str(snippet.text.trim());
        rendered.push('\n');
    }
    Some(rendered)
}

fn structured_evidence_snippets(value: &Value) -> Vec<EvidenceSnippet> {
    let evidence_by_ref = evidence_text_by_supported_ref(value);
    let mut snippets = Vec::new();

    for relation in proof_path(value) {
        let is_support = relation
            .get("rel")
            .and_then(Value::as_str)
            .is_some_and(|rel| rel == "supports_answer" || rel == "supports");
        if !is_support {
            continue;
        }
        let Some(ref_id) = relation.get("from").and_then(Value::as_str) else {
            continue;
        };
        if !looks_like_turn_ref(ref_id) {
            continue;
        }
        let evidence = evidence_by_ref.get(ref_id);
        let relation_evidence = relation
            .get("evidence")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let text = evidence
            .map(|snippet| snippet.text.as_str())
            .filter(|text| !text.trim().is_empty())
            .unwrap_or(relation_evidence)
            .trim();
        if text.is_empty() {
            continue;
        }
        snippets.push(EvidenceSnippet {
            ref_id: ref_id.to_string(),
            text: text.to_string(),
            source: evidence.and_then(|snippet| snippet.source.clone()),
            time: evidence.and_then(|snippet| snippet.time.clone()),
            sequence: relation.get("sequence").and_then(Value::as_u64),
            why: relation
                .get("why")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            confidence: relation
                .get("confidence")
                .and_then(Value::as_str)
                .map(ToString::to_string),
        });
    }

    if snippets.is_empty() {
        for reason in value
            .get("because")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[])
        {
            let Some(ref_id) = reason.get("ref").and_then(Value::as_str) else {
                continue;
            };
            if !looks_like_turn_ref(ref_id) {
                continue;
            }
            let text = reason
                .get("evidence")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim();
            if text.is_empty() {
                continue;
            }
            snippets.push(EvidenceSnippet {
                ref_id: ref_id.to_string(),
                text: text.to_string(),
                source: None,
                time: None,
                sequence: None,
                why: reason
                    .get("claim")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                confidence: None,
            });
        }
    }

    if snippets.is_empty() {
        snippets.extend(evidence_by_ref.into_values());
    }

    deduplicate_snippets(snippets)
}

fn evidence_text_by_supported_ref(value: &Value) -> BTreeMap<String, EvidenceSnippet> {
    let mut evidence_by_ref = BTreeMap::new();
    for evidence in proof_evidence(value) {
        let text = evidence
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim();
        if text.is_empty() {
            continue;
        }
        let source = evidence
            .get("source")
            .and_then(Value::as_str)
            .map(ToString::to_string);
        let time = evidence
            .get("time")
            .and_then(Value::as_str)
            .map(ToString::to_string);
        let supports = evidence
            .get("supports")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        for support in supports {
            let Some(ref_id) = support.as_str() else {
                continue;
            };
            if !looks_like_turn_ref(ref_id) {
                continue;
            }
            evidence_by_ref
                .entry(ref_id.to_string())
                .or_insert_with(|| EvidenceSnippet {
                    ref_id: ref_id.to_string(),
                    text: text.to_string(),
                    source: source.clone(),
                    time: time.clone(),
                    sequence: None,
                    why: None,
                    confidence: None,
                });
        }
    }
    evidence_by_ref
}

fn proof_path(value: &Value) -> &[Value] {
    value
        .get("proof")
        .and_then(|proof| proof.get("path"))
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn proof_evidence(value: &Value) -> &[Value] {
    value
        .get("proof")
        .and_then(|proof| proof.get("evidence"))
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn deduplicate_snippets(snippets: Vec<EvidenceSnippet>) -> Vec<EvidenceSnippet> {
    let mut deduplicated = BTreeMap::<String, EvidenceSnippet>::new();
    for snippet in snippets {
        deduplicated
            .entry(snippet.ref_id.clone())
            .and_modify(|existing| {
                if existing.sequence.is_none() {
                    existing.sequence = snippet.sequence;
                }
                if existing.why.is_none() {
                    existing.why = snippet.why.clone();
                }
                if existing.confidence.is_none() {
                    existing.confidence = snippet.confidence.clone();
                }
            })
            .or_insert(snippet);
    }
    deduplicated.into_values().collect()
}

fn looks_like_turn_ref(value: &str) -> bool {
    value.starts_with("turn:") && !value.contains(' ') && value.len() <= 240
}

fn normalize_reader_output(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .trim()
        .trim_start_matches("Answer:")
        .trim()
        .to_string()
}

fn validate_matching_item(
    expected: &LongMemEvalExpected,
    run_item: &RunItem,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    if expected.question_id != run_item.question_id {
        return Err(format!(
            "question_id mismatch: expected={} run={}",
            expected.question_id, run_item.question_id
        )
        .into());
    }
    if expected.question_type != run_item.question_type {
        return Err(format!(
            "question_type mismatch for {}: expected={} run={}",
            expected.question_id, expected.question_type, run_item.question_type
        )
        .into());
    }
    Ok(())
}

fn summarize_reader_run(
    args: &Args,
    endpoint: String,
    model: String,
    provider: LlmProvider,
    elapsed_ms: u128,
    results: &[ReaderItemResult],
) -> Result<ReaderSummary, Box<dyn Error + Send + Sync>> {
    let mut question_types = BTreeMap::<String, ReaderQuestionTypeSummary>::new();
    for result in results {
        let summary = question_types
            .entry(result.question_type.clone())
            .or_default();
        summary.count += 1;
        if result.lexical_answer_hit {
            summary.lexical_answer_hits += 1;
        }
    }

    Ok(ReaderSummary {
        benchmark: "LongMemEval",
        reader: "longmemeval-kmp-reader-v1",
        generated_at_unix_seconds: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
        artifacts: args.artifacts.display().to_string(),
        run: args.run.display().to_string(),
        endpoint,
        model,
        provider: provider_label(provider),
        total_items: results.len(),
        lexical_answer_items: results
            .iter()
            .filter(|result| !normalize_answer_text(&result.expected_answer).is_empty())
            .count(),
        lexical_answer_hits: results
            .iter()
            .filter(|result| result.lexical_answer_hit)
            .count(),
        prompt_tokens: results.iter().map(|result| result.prompt_tokens).sum(),
        completion_tokens: results.iter().map(|result| result.completion_tokens).sum(),
        elapsed_ms,
        question_types,
    })
}

fn read_expected(
    path: &Path,
    limit: Option<usize>,
) -> Result<Vec<LongMemEvalExpected>, Box<dyn Error + Send + Sync>> {
    read_jsonl(path, limit)?
        .into_iter()
        .map(serde_json::from_value)
        .collect::<Result<Vec<_>, _>>()
        .map_err(Into::into)
}

fn read_run_items(
    path: &Path,
    limit: Option<usize>,
) -> Result<Vec<RunItem>, Box<dyn Error + Send + Sync>> {
    read_jsonl(path, limit)?
        .into_iter()
        .map(serde_json::from_value)
        .collect::<Result<Vec<_>, _>>()
        .map_err(Into::into)
}

fn read_jsonl(
    path: &Path,
    limit: Option<usize>,
) -> Result<Vec<Value>, Box<dyn Error + Send + Sync>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut values = Vec::new();
    for (line_index, line) in reader.lines().enumerate() {
        if limit.is_some_and(|limit| values.len() >= limit) {
            break;
        }
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

fn answer_contains_expected(expected_answer: &Value, ask_answer: Option<&str>) -> bool {
    let Some(ask_answer) = ask_answer else {
        return false;
    };
    let expected = normalize_answer_text(expected_answer);
    let expected = normalize_for_lexical_match(&expected);
    let observed = normalize_for_lexical_match(ask_answer);
    !expected.is_empty() && observed.contains(&expected)
}

fn normalize_answer_text(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Array(values) => values
            .iter()
            .map(normalize_answer_text)
            .filter(|value| !value.trim().is_empty())
            .collect::<Vec<_>>()
            .join(" "),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::Object(_) => value.to_string(),
    }
}

fn normalize_for_lexical_match(value: &str) -> String {
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

fn resolve_required(
    cli_value: Option<&str>,
    env_key: &str,
    flag: &str,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    cli_value
        .map(ToString::to_string)
        .or_else(|| env::var(env_key).ok())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{flag} or {env_key} is required").into())
}

fn detect_provider_from_model(model: &str) -> LlmProvider {
    if model.starts_with("claude") {
        LlmProvider::Anthropic
    } else if model.starts_with("gpt-5") || model.starts_with("o3") || model.starts_with("o4") {
        LlmProvider::OpenAINew
    } else {
        LlmProvider::OpenAI
    }
}

fn parse_provider(value: &str) -> Result<LlmProvider, Box<dyn Error + Send + Sync>> {
    match value.trim().to_ascii_lowercase().as_str() {
        "openai" => Ok(LlmProvider::OpenAI),
        "openai-new" | "openai_new" => Ok(LlmProvider::OpenAINew),
        "anthropic" => Ok(LlmProvider::Anthropic),
        other => Err(format!(
            "unsupported provider `{other}`; use openai, openai-new, or anthropic"
        )
        .into()),
    }
}

fn provider_label(provider: LlmProvider) -> &'static str {
    match provider {
        LlmProvider::OpenAI => "openai",
        LlmProvider::OpenAINew => "openai-new",
        LlmProvider::Anthropic => "anthropic",
    }
}

fn parse_args(
    mut args: impl Iterator<Item = String>,
) -> Result<Args, Box<dyn Error + Send + Sync>> {
    let mut artifacts = None;
    let mut run = None;
    let mut output = None;
    let mut endpoint = None;
    let mut model = None;
    let mut provider = None;
    let mut api_key_env = "LLM_API_KEY".to_string();
    let mut max_tokens = 256u32;
    let mut temperature = 0.0f64;
    let mut limit = None;
    let mut force = false;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--artifacts" => artifacts = Some(PathBuf::from(required_flag_value(&mut args, &arg)?)),
            "--run" => run = Some(PathBuf::from(required_flag_value(&mut args, &arg)?)),
            "--output" => output = Some(PathBuf::from(required_flag_value(&mut args, &arg)?)),
            "--endpoint" => endpoint = Some(required_flag_value(&mut args, &arg)?),
            "--model" => model = Some(required_flag_value(&mut args, &arg)?),
            "--provider" => {
                provider = Some(parse_provider(&required_flag_value(&mut args, &arg)?)?)
            }
            "--api-key-env" => api_key_env = required_flag_value(&mut args, &arg)?,
            "--max-tokens" => {
                let value = required_flag_value(&mut args, &arg)?;
                max_tokens = value
                    .parse::<u32>()
                    .map_err(|error| format!("invalid --max-tokens value `{value}`: {error}"))?;
            }
            "--temperature" => {
                let value = required_flag_value(&mut args, &arg)?;
                temperature = value
                    .parse::<f64>()
                    .map_err(|error| format!("invalid --temperature value `{value}`: {error}"))?;
            }
            "--limit" => {
                let value = required_flag_value(&mut args, &arg)?;
                limit = Some(
                    value
                        .parse::<usize>()
                        .map_err(|error| format!("invalid --limit value `{value}`: {error}"))?,
                );
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
        endpoint,
        model,
        provider,
        api_key_env,
        max_tokens,
        temperature,
        limit,
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
        "Usage: longmemeval_kmp_reader --artifacts <adapter-output-dir> --run <runner-output-dir> --output <reader-output-dir> --endpoint <chat-completions-url> --model <model> [--provider openai|openai-new|anthropic] [--api-key-env LLM_API_KEY] [--max-tokens N] [--temperature F] [--limit N] [--force]"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_contains_task_specific_instruction_without_gold_answer() {
        let prompt = build_reader_prompt(
            "multi-session",
            "How many appointments?",
            false,
            "I visited a dentist. I visited an ENT specialist.",
        );

        assert!(prompt.contains("Aggregate all relevant evidence"));
        assert!(prompt.contains("How many appointments?"));
        assert!(prompt.contains("I visited a dentist"));
        assert!(!prompt.contains("expected"));
    }

    #[test]
    fn normalizes_reader_output() {
        assert_eq!(normalize_reader_output("  \"Answer: 3 days\"  "), "3 days");
    }

    #[test]
    fn preference_prompt_requests_personalized_assistant_response() {
        let prompt = build_reader_prompt(
            "single-session-preference",
            "Can you recommend some resources?",
            false,
            "The user wants Adobe Premiere Pro advanced settings.",
        );

        assert!(prompt.contains("personalized assistant response"));
        assert!(prompt.contains("not as a meta-summary"));
        assert!(prompt.contains("every distinct user-specific preference"));
        assert!(prompt.contains("preference-constrained criteria"));
        assert!(prompt.contains("maintenance concern"));
        assert!(prompt.contains("Can you recommend some resources?"));
    }

    #[test]
    fn structured_evidence_orders_updates_by_sequence() {
        let run_item = RunItem {
            question_id: "q-update".to_string(),
            question_type: "knowledge-update".to_string(),
            evidence_hit: "full".to_string(),
            ask_answer: Some("fallback".to_string()),
            ask_content: Some(json!({
                "proof": {
                    "path": [
                            {
                                "from": "turn:q-update:s1:1",
                                "to": "question:q-update",
                                "rel": "supports_answer",
                                "class": "evidential",
                                "why": "older value",
                            "evidence": "Yoga twice a week.",
                            "confidence": "high",
                            "sequence": 1
                        },
                            {
                                "from": "turn:q-update:s2:1",
                                "to": "question:q-update",
                                "rel": "supports_answer",
                                "class": "evidential",
                            "why": "updated value",
                            "evidence": "Yoga three times a week.",
                            "confidence": "high",
                            "sequence": 2
                        }
                    ],
                    "evidence": []
                }
            })),
        };

        let evidence = recovered_evidence_text(&run_item).expect("structured evidence");
        assert!(evidence.contains("sequence=1"));
        assert!(evidence.contains("sequence=2"));
        assert!(
            evidence
                .find("Yoga twice a week")
                .expect("old value exists")
                < evidence
                    .find("Yoga three times a week")
                    .expect("new value exists")
        );
    }
}
