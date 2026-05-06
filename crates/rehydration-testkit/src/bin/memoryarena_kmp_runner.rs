use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::error::Error;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use rehydration_mcp::KernelMcpServer;
use rehydration_testkit::MemoryArenaExpected;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

type TaskKey = (String, String);
type TaskSubtaskKey = (String, String, usize);

#[derive(Debug, Clone, PartialEq, Eq)]
struct Args {
    artifacts: PathBuf,
    endpoint: Option<String>,
    output: PathBuf,
    limit_tasks: Option<usize>,
    force: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct MemoryArenaEvent {
    event: String,
    task_id: String,
    task_type: String,
    #[serde(default)]
    category: Option<String>,
    event_index: usize,
    phase: String,
    #[serde(default)]
    subtask_index: Option<usize>,
    about: String,
    tool: String,
    arguments: Value,
}

#[derive(Debug, Serialize)]
struct EventResult {
    event: String,
    task_id: String,
    task_type: String,
    category: Option<String>,
    event_index: usize,
    phase: String,
    subtask_index: Option<usize>,
    about: String,
    tool: String,
    elapsed_ms: u128,
    success: bool,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct AskResult {
    task_id: String,
    task_type: String,
    category: Option<String>,
    subtask_index: usize,
    about: String,
    question: String,
    expected_answer: Value,
    current_question_ref: String,
    expected_answer_ref: String,
    allowed_known_at_refs: Vec<String>,
    observed_refs: Vec<String>,
    observed_allowed_refs: Vec<String>,
    unexpected_refs: Vec<String>,
    missing_allowed_refs: Vec<String>,
    current_question_observed: bool,
    future_answer_leaked: bool,
    known_at_clean: bool,
    lexical_answer_hit: bool,
    ask_answer: Option<String>,
    ask_content: Value,
    ask_elapsed_ms: u128,
}

#[derive(Debug, Serialize)]
struct RunSummary {
    benchmark: &'static str,
    runner: &'static str,
    generated_at_unix_seconds: u64,
    artifacts: String,
    endpoint: String,
    total_events: usize,
    ingest_events: usize,
    ask_events: usize,
    successful_events: usize,
    failed_events: usize,
    known_at_clean_asks: usize,
    future_answer_leaks: usize,
    current_question_observed: usize,
    lexical_answer_hits: usize,
    unexpected_ref_asks: usize,
    missing_allowed_ref_asks: usize,
    elapsed_ms: u128,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let args = parse_args(env::args().skip(1))?;
    ensure_output_dir(&args.output, args.force)?;

    let events = read_events(&args.artifacts.join("events.jsonl"), args.limit_tasks)?;
    let expected = read_expected(&args.artifacts.join("expected.jsonl"), args.limit_tasks)?;
    let expected_by_ask = expected_by_ask_key(expected)?;
    validate_events_expected_alignment(&events, &expected_by_ask)?;

    let server = match args.endpoint.as_deref() {
        Some(endpoint) => KernelMcpServer::grpc(endpoint),
        None => KernelMcpServer::try_from_env()
            .map_err(|error| format!("failed to configure MCP gRPC backend from env: {error}"))?,
    };
    let endpoint_label = args
        .endpoint
        .clone()
        .or_else(|| env::var("REHYDRATION_KERNEL_GRPC_ENDPOINT").ok())
        .unwrap_or_else(|| "env".to_string());

    let started = Instant::now();
    let mut request_id = 1u64;
    let mut event_results = Vec::new();
    let mut ask_results = Vec::new();

    for event in &events {
        validate_event(event)?;
        let event_started = Instant::now();
        let response = call_mcp_tool(&server, request_id, &event.tool, &event.arguments).await;
        request_id = request_id.checked_add(1).ok_or("request id overflow")?;
        let elapsed_ms = event_started.elapsed().as_millis();

        match response {
            Ok(response) => {
                let content_result = assert_tool_success(&response, &event.tool, event);
                match content_result {
                    Ok(content) => {
                        if event.event == "ask" {
                            let subtask_index = event.subtask_index.ok_or_else(|| {
                                format!(
                                    "ask event {} for task_type {} task {} has no subtask_index",
                                    event.event_index, event.task_type, event.task_id
                                )
                            })?;
                            let expected = expected_by_ask
                                .get(&ask_key(&event.task_type, &event.task_id, subtask_index))
                                .ok_or_else(|| {
                                    format!(
                                        "missing expected row for task_type {} task {} subtask {}",
                                        event.task_type, event.task_id, subtask_index
                                    )
                                })?;
                            ask_results
                                .push(build_ask_result(event, expected, content, elapsed_ms));
                        }
                        event_results.push(success_event_result(event, elapsed_ms));
                    }
                    Err(error) => {
                        event_results.push(failed_event_result(event, elapsed_ms, error));
                    }
                }
            }
            Err(error) => {
                event_results.push(failed_event_result(event, elapsed_ms, error.to_string()));
            }
        }
    }

    let summary = summarize_run(
        &args,
        endpoint_label,
        started.elapsed().as_millis(),
        &event_results,
        &ask_results,
    )?;
    write_jsonl(
        &args.output.join("event_results.jsonl"),
        event_results.iter().map(serde_json::to_value),
    )?;
    write_jsonl(
        &args.output.join("results.jsonl"),
        ask_results.iter().map(serde_json::to_value),
    )?;
    write_jsonl(
        &args.output.join("hypotheses.jsonl"),
        ask_results.iter().map(|item| {
            Ok(json!({
                "task_id": item.task_id,
                "subtask_index": item.subtask_index,
                "hypothesis": item.ask_answer.as_deref().unwrap_or_default()
            }))
        }),
    )?;
    write_json_pretty(&args.output.join("summary.json"), &summary)?;

    println!("{}", serde_json::to_string_pretty(&summary)?);
    if summary.failed_events > 0 {
        return Err(format!(
            "MemoryArena KMP runner failed: {} event(s) failed",
            summary.failed_events
        )
        .into());
    }
    Ok(())
}

async fn call_mcp_tool(
    server: &KernelMcpServer,
    id: u64,
    name: &str,
    arguments: &Value,
) -> Result<Value, Box<dyn Error + Send + Sync>> {
    let request = json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": {
            "name": name,
            "arguments": arguments
        }
    });
    let response = server
        .handle_json_line(&request.to_string())
        .await
        .ok_or_else(|| format!("MCP tool `{name}` returned no JSON-RPC response"))?;
    let value = serde_json::from_str::<Value>(&response)?;
    if let Some(error) = value.get("error") {
        return Err(format!("MCP tool `{name}` returned JSON-RPC error: {error}").into());
    }
    Ok(value)
}

fn assert_tool_success<'a>(
    response: &'a Value,
    tool: &str,
    event: &MemoryArenaEvent,
) -> Result<&'a Value, String> {
    let result = response
        .get("result")
        .ok_or_else(|| format!("{tool} event {} returned no result", event.event_index))?;
    if result
        .get("isError")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Err(format!(
            "{tool} event {} for task {} failed: {result}",
            event.event_index, event.task_id
        ));
    }
    result.get("structuredContent").ok_or_else(|| {
        format!(
            "{tool} event {} for task {} returned no structuredContent",
            event.event_index, event.task_id
        )
    })
}

fn build_ask_result(
    event: &MemoryArenaEvent,
    expected: &MemoryArenaExpected,
    ask_content: &Value,
    ask_elapsed_ms: u128,
) -> AskResult {
    let observed_refs = collect_memoryarena_refs(ask_content);
    let observed_entry_refs = observed_refs
        .iter()
        .filter(|reference| is_memoryarena_entry_ref(reference))
        .cloned()
        .collect::<BTreeSet<_>>();
    let allowed = expected
        .available_ref_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let observed_allowed_refs = observed_entry_refs
        .intersection(&allowed)
        .cloned()
        .collect::<Vec<_>>();
    let unexpected_refs = observed_entry_refs
        .difference(&allowed)
        .cloned()
        .collect::<Vec<_>>();
    let missing_allowed_refs = allowed
        .difference(&observed_entry_refs)
        .cloned()
        .collect::<Vec<_>>();
    let current_question_observed = observed_entry_refs.contains(&expected.current_question_ref);
    let future_answer_leaked = observed_entry_refs.contains(&expected.expected_answer_ref);
    let known_at_clean = unexpected_refs.is_empty();
    let ask_answer = ask_content
        .get("answer")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let lexical_answer_hit = answer_contains_expected(&expected.answer, ask_answer.as_deref());

    AskResult {
        task_id: expected.task_id.clone(),
        task_type: expected.task_type.clone(),
        category: expected.category.clone().or_else(|| event.category.clone()),
        subtask_index: expected.subtask_index,
        about: expected.about.clone(),
        question: expected.question.clone(),
        expected_answer: expected.answer.clone(),
        current_question_ref: expected.current_question_ref.clone(),
        expected_answer_ref: expected.expected_answer_ref.clone(),
        allowed_known_at_refs: expected.available_ref_ids.clone(),
        observed_refs: observed_refs.into_iter().collect(),
        observed_allowed_refs,
        unexpected_refs,
        missing_allowed_refs,
        current_question_observed,
        future_answer_leaked,
        known_at_clean,
        lexical_answer_hit,
        ask_answer,
        ask_content: ask_content.clone(),
        ask_elapsed_ms,
    }
}

fn collect_memoryarena_refs(value: &Value) -> BTreeSet<String> {
    let mut refs = BTreeSet::new();
    collect_memoryarena_refs_from_field(value, None, &mut refs);
    refs
}

fn collect_memoryarena_refs_from_field(
    value: &Value,
    field: Option<&str>,
    refs: &mut BTreeSet<String>,
) {
    match value {
        Value::String(value)
            if field_allows_memory_ref(field) && looks_like_memoryarena_ref(value) =>
        {
            refs.insert(value.to_string());
        }
        Value::Array(values) => {
            for value in values {
                collect_memoryarena_refs_from_field(value, field, refs);
            }
        }
        Value::Object(object) => {
            for (key, value) in object {
                collect_memoryarena_refs_from_field(value, Some(key), refs);
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

fn looks_like_memoryarena_ref(value: &str) -> bool {
    value.starts_with("memoryarena:") && !value.contains(' ') && value.len() <= 320
}

fn is_memoryarena_entry_ref(value: &str) -> bool {
    value.contains(":subtask:") || value.contains(":background")
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

fn validate_event(event: &MemoryArenaEvent) -> Result<(), Box<dyn Error + Send + Sync>> {
    match event.event.as_str() {
        "ingest" if event.tool == "kernel_ingest" => Ok(()),
        "ask" if event.tool == "kernel_ask" && event.subtask_index.is_some() => Ok(()),
        "ask" => Err(format!(
            "MemoryArena ask event {} for task {} must use kernel_ask and include subtask_index",
            event.event_index, event.task_id
        )
        .into()),
        other => Err(format!(
            "unsupported MemoryArena event `{other}` at event_index {} for task {}",
            event.event_index, event.task_id
        )
        .into()),
    }
}

fn success_event_result(event: &MemoryArenaEvent, elapsed_ms: u128) -> EventResult {
    EventResult {
        event: event.event.clone(),
        task_id: event.task_id.clone(),
        task_type: event.task_type.clone(),
        category: event.category.clone(),
        event_index: event.event_index,
        phase: event.phase.clone(),
        subtask_index: event.subtask_index,
        about: event.about.clone(),
        tool: event.tool.clone(),
        elapsed_ms,
        success: true,
        error: None,
    }
}

fn failed_event_result(event: &MemoryArenaEvent, elapsed_ms: u128, error: String) -> EventResult {
    EventResult {
        event: event.event.clone(),
        task_id: event.task_id.clone(),
        task_type: event.task_type.clone(),
        category: event.category.clone(),
        event_index: event.event_index,
        phase: event.phase.clone(),
        subtask_index: event.subtask_index,
        about: event.about.clone(),
        tool: event.tool.clone(),
        elapsed_ms,
        success: false,
        error: Some(error),
    }
}

fn summarize_run(
    args: &Args,
    endpoint: String,
    elapsed_ms: u128,
    event_results: &[EventResult],
    ask_results: &[AskResult],
) -> Result<RunSummary, Box<dyn Error + Send + Sync>> {
    Ok(RunSummary {
        benchmark: "MemoryArena",
        runner: "memoryarena-kmp-runner-v1",
        generated_at_unix_seconds: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
        artifacts: args.artifacts.display().to_string(),
        endpoint,
        total_events: event_results.len(),
        ingest_events: event_results
            .iter()
            .filter(|event| event.event == "ingest")
            .count(),
        ask_events: event_results
            .iter()
            .filter(|event| event.event == "ask")
            .count(),
        successful_events: event_results.iter().filter(|event| event.success).count(),
        failed_events: event_results.iter().filter(|event| !event.success).count(),
        known_at_clean_asks: ask_results
            .iter()
            .filter(|result| result.known_at_clean)
            .count(),
        future_answer_leaks: ask_results
            .iter()
            .filter(|result| result.future_answer_leaked)
            .count(),
        current_question_observed: ask_results
            .iter()
            .filter(|result| result.current_question_observed)
            .count(),
        lexical_answer_hits: ask_results
            .iter()
            .filter(|result| result.lexical_answer_hit)
            .count(),
        unexpected_ref_asks: ask_results
            .iter()
            .filter(|result| !result.unexpected_refs.is_empty())
            .count(),
        missing_allowed_ref_asks: ask_results
            .iter()
            .filter(|result| !result.missing_allowed_refs.is_empty())
            .count(),
        elapsed_ms,
    })
}

fn expected_by_ask_key(
    expected: Vec<MemoryArenaExpected>,
) -> Result<BTreeMap<TaskSubtaskKey, MemoryArenaExpected>, Box<dyn Error + Send + Sync>> {
    let mut by_key = BTreeMap::new();
    for item in expected {
        let key = ask_key(&item.task_type, &item.task_id, item.subtask_index);
        if by_key.insert(key.clone(), item).is_some() {
            return Err(format!(
                "duplicate expected row for task_type {} task {} subtask {}",
                key.0, key.1, key.2
            )
            .into());
        }
    }
    Ok(by_key)
}

fn validate_events_expected_alignment(
    events: &[MemoryArenaEvent],
    expected_by_ask: &BTreeMap<TaskSubtaskKey, MemoryArenaExpected>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut ask_keys = BTreeSet::new();
    for event in events.iter().filter(|event| event.event == "ask") {
        let subtask_index = event.subtask_index.ok_or_else(|| {
            format!(
                "ask event {} for task_type {} task {} has no subtask_index",
                event.event_index, event.task_type, event.task_id
            )
        })?;
        let key = ask_key(&event.task_type, &event.task_id, subtask_index);
        if !ask_keys.insert(key.clone()) {
            return Err(format!(
                "duplicate ask event for task_type {} task {} subtask {}",
                key.0, key.1, key.2
            )
            .into());
        }
        if !expected_by_ask.contains_key(&key) {
            return Err(format!(
                "missing expected row for task_type {} task {} subtask {}",
                key.0, key.1, key.2
            )
            .into());
        }
    }

    for key in expected_by_ask.keys() {
        if !ask_keys.contains(key) {
            return Err(format!(
                "expected row has no ask event for task_type {} task {} subtask {}",
                key.0, key.1, key.2
            )
            .into());
        }
    }

    Ok(())
}

fn read_events(
    path: &Path,
    limit_tasks: Option<usize>,
) -> Result<Vec<MemoryArenaEvent>, Box<dyn Error + Send + Sync>> {
    let selected_task_keys = selected_task_keys(path, limit_tasks)?;
    let events = read_jsonl(path)?
        .into_iter()
        .map(serde_json::from_value)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|event: &MemoryArenaEvent| {
            selected_task_keys
                .as_ref()
                .is_none_or(|keys| keys.contains(&task_key(&event.task_type, &event.task_id)))
        })
        .collect::<Vec<_>>();
    Ok(events)
}

fn read_expected(
    path: &Path,
    limit_tasks: Option<usize>,
) -> Result<Vec<MemoryArenaExpected>, Box<dyn Error + Send + Sync>> {
    let selected_task_keys = selected_task_keys_from_expected(path, limit_tasks)?;
    let expected = read_jsonl(path)?
        .into_iter()
        .map(serde_json::from_value)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|expected: &MemoryArenaExpected| {
            selected_task_keys
                .as_ref()
                .is_none_or(|keys| keys.contains(&task_key(&expected.task_type, &expected.task_id)))
        })
        .collect::<Vec<_>>();
    Ok(expected)
}

fn selected_task_keys(
    path: &Path,
    limit_tasks: Option<usize>,
) -> Result<Option<BTreeSet<TaskKey>>, Box<dyn Error + Send + Sync>> {
    let Some(limit) = limit_tasks else {
        return Ok(None);
    };
    let mut selected = BTreeSet::new();
    for value in read_jsonl(path)? {
        let task_type = required_string(&value, "task_type")?;
        let task_id = required_string(&value, "task_id")?;
        selected.insert(task_key(&task_type, &task_id));
        if selected.len() >= limit {
            break;
        }
    }
    Ok(Some(selected))
}

fn selected_task_keys_from_expected(
    path: &Path,
    limit_tasks: Option<usize>,
) -> Result<Option<BTreeSet<TaskKey>>, Box<dyn Error + Send + Sync>> {
    let Some(limit) = limit_tasks else {
        return Ok(None);
    };
    let mut selected = BTreeSet::new();
    for value in read_jsonl(path)? {
        let task_type = required_string(&value, "task_type")?;
        let task_id = required_string(&value, "task_id")?;
        selected.insert(task_key(&task_type, &task_id));
        if selected.len() >= limit {
            break;
        }
    }
    Ok(Some(selected))
}

fn task_key(task_type: &str, task_id: &str) -> TaskKey {
    (task_type.to_string(), task_id.to_string())
}

fn ask_key(task_type: &str, task_id: &str, subtask_index: usize) -> TaskSubtaskKey {
    (task_type.to_string(), task_id.to_string(), subtask_index)
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
    let mut endpoint = None;
    let mut output = None;
    let mut limit_tasks = None;
    let mut force = false;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--artifacts" => artifacts = Some(PathBuf::from(required_flag_value(&mut args, &arg)?)),
            "--endpoint" => endpoint = Some(required_flag_value(&mut args, &arg)?),
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
        endpoint,
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
        "Usage: memoryarena_kmp_runner --artifacts <adapter-output-dir> --output <run-dir> [--endpoint http://host] [--limit-tasks N] [--force]"
    );
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn collects_memoryarena_refs_recursively() {
        let refs = collect_memoryarena_refs(&json!({
            "answer": "text memoryarena:not-a-ref with spaces",
            "because": [
                {"ref": "memoryarena:task_type:x:task:1:subtask:1:question"}
            ],
            "proof": {
                "path": [
                    {
                        "from": "memoryarena:task_type:x:task:1:subtask:1:answer",
                        "to": "memoryarena:task_type:x:task:1:subtask:1:question"
                    }
                ]
            }
        }));

        assert!(refs.contains("memoryarena:task_type:x:task:1:subtask:1:question"));
        assert!(refs.contains("memoryarena:task_type:x:task:1:subtask:1:answer"));
        assert!(!refs.contains("memoryarena:not-a-ref with spaces"));
    }

    #[test]
    fn ask_result_detects_future_answer_leak() {
        let event = MemoryArenaEvent {
            event: "ask".to_string(),
            task_id: "1".to_string(),
            task_type: "progressive_search".to_string(),
            category: None,
            event_index: 3,
            phase: "ask".to_string(),
            subtask_index: Some(1),
            about: "memoryarena:task_type:progressive_search:task:1".to_string(),
            tool: "kernel_ask".to_string(),
            arguments: json!({}),
        };
        let expected = MemoryArenaExpected {
            task_id: "1".to_string(),
            task_type: "progressive_search".to_string(),
            category: None,
            subtask_index: 1,
            question: "Q?".to_string(),
            answer: json!("A"),
            about: "memoryarena:task_type:progressive_search:task:1".to_string(),
            current_question_ref:
                "memoryarena:task_type:progressive_search:task:1:subtask:1:question".to_string(),
            expected_answer_ref: "memoryarena:task_type:progressive_search:task:1:subtask:1:answer"
                .to_string(),
            available_ref_ids: vec![
                "memoryarena:task_type:progressive_search:task:1:subtask:1:question".to_string(),
            ],
        };

        let result = build_ask_result(
            &event,
            &expected,
            &json!({
                "answer": "A",
                "because": [
                    {"ref": "memoryarena:task_type:progressive_search:task:1:subtask:1:answer"}
                ]
            }),
            12,
        );

        assert!(result.future_answer_leaked);
        assert!(!result.known_at_clean);
        assert_eq!(
            result.unexpected_refs,
            vec!["memoryarena:task_type:progressive_search:task:1:subtask:1:answer"]
        );
    }

    #[test]
    fn expected_alignment_keys_include_task_type() {
        let expected = vec![
            expected_fixture("progressive_search", "1", 1),
            expected_fixture("formal_reasoning_phys", "1", 1),
        ];
        let expected_by_ask =
            expected_by_ask_key(expected).expect("same task_id across configs is valid");
        let events = vec![
            ask_event_fixture("progressive_search", "1", 1, 1),
            ask_event_fixture("formal_reasoning_phys", "1", 1, 2),
        ];

        validate_events_expected_alignment(&events, &expected_by_ask)
            .expect("events should align by task_type plus task_id");
    }

    fn ask_event_fixture(
        task_type: &str,
        task_id: &str,
        subtask_index: usize,
        event_index: usize,
    ) -> MemoryArenaEvent {
        MemoryArenaEvent {
            event: "ask".to_string(),
            task_id: task_id.to_string(),
            task_type: task_type.to_string(),
            category: None,
            event_index,
            phase: "ask".to_string(),
            subtask_index: Some(subtask_index),
            about: format!("memoryarena:task_type:{task_type}:task:{task_id}"),
            tool: "kernel_ask".to_string(),
            arguments: json!({}),
        }
    }

    fn expected_fixture(
        task_type: &str,
        task_id: &str,
        subtask_index: usize,
    ) -> MemoryArenaExpected {
        MemoryArenaExpected {
            task_id: task_id.to_string(),
            task_type: task_type.to_string(),
            category: None,
            subtask_index,
            question: format!("question {subtask_index}"),
            answer: json!("answer"),
            about: format!("memoryarena:task_type:{task_type}:task:{task_id}"),
            current_question_ref: format!(
                "memoryarena:task_type:{task_type}:task:{task_id}:subtask:{subtask_index}:question"
            ),
            expected_answer_ref: format!(
                "memoryarena:task_type:{task_type}:task:{task_id}:subtask:{subtask_index}:answer"
            ),
            available_ref_ids: vec![format!(
                "memoryarena:task_type:{task_type}:task:{task_id}:subtask:{subtask_index}:question"
            )],
        }
    }
}
