use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::error::Error;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::{Value, json};

use underpass_operator_shared_domain::{
    operator_action_contract_diagnostic as kernel_operator_action_contract_diagnostic,
    operator_allowed_full_tools as kernel_operator_allowed_full_tools,
    operator_allowed_read_tools as kernel_operator_allowed_read_tools,
    operator_allowed_write_tools as kernel_operator_allowed_write_tools,
    operator_allowed_writer_pre_read_tools as kernel_operator_allowed_writer_pre_read_tools,
};
use underpass_operator_synthetic_domain::{ReadAgentDimension, ReadIntent, ReadTopicTitle};
use underpass_operator_synthetic_infra::conformance_fixtures::{
    action_arguments_from_json, read_api_mcp_topics_from_json, read_api_mcp_variants_from_json,
    writer_exec_topics_from_json, writer_pre_read_topics_from_json,
    writer_pre_read_variants_from_json,
};

const EXPORTER: &str = "kernel-operator-conformance-trajectory-export-v1";
const SCHEMA_VERSION: &str = "kernel-operator-trajectory-v1";
const DEFAULT_RUN_ID: &str = "kmp-operator-conformance-v5";
const DEFAULT_CONTEXT_CHARS: usize = 12_000;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Args {
    output: PathBuf,
    run_id: String,
    suite: String,
    force: bool,
}

#[derive(Debug, Serialize)]
struct TrajectoryItem {
    schema_version: &'static str,
    run_id: String,
    task_family: String,
    mode: String,
    source: String,
    about: String,
    step_id: String,
    step_index: usize,
    goal: String,
    visible_state: Value,
    allowed_tools: Vec<String>,
    target_action: Value,
    observed_outcome: Option<Value>,
    quality: Value,
}

#[derive(Debug, Serialize)]
struct ExportSummary {
    exporter: &'static str,
    schema_version: &'static str,
    generated_at_unix_seconds: u64,
    suite: String,
    run_id: String,
    output: String,
    trajectories: usize,
    modes: BTreeMap<String, usize>,
    task_families: BTreeMap<String, usize>,
    target_actions: BTreeMap<String, usize>,
    contract_validation_failures: usize,
    contract_validation_failure_phases: BTreeMap<String, usize>,
    notes: Vec<String>,
}

fn fixture_or_panic<T, E: std::fmt::Display>(fixture_name: &str, result: Result<T, E>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("invalid operator conformance fixture `{fixture_name}`: {error}"),
    }
}

fn action_arguments_fixture(fixture_name: &str, raw: &str) -> Value {
    fixture_or_panic(fixture_name, action_arguments_from_json(raw)).into_value()
}

fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let args = parse_args(env::args().skip(1))?;
    ensure_output_dir(&args.output, args.force)?;

    let trajectories = match args.suite.as_str() {
        "golden" => conformance_trajectories(&args.run_id),
        "golden-v3" => golden_v3_trajectories(&args.run_id),
        "golden-v4" => golden_v4_trajectories(&args.run_id),
        "read-generalization" => read_generalization_trajectories(&args.run_id),
        "read-rare-v1" => read_rare_expansion_trajectories(&args.run_id),
        "read-api-mcp-v1" => read_api_mcp_v1_trajectories(&args.run_id),
        "writer-pre-read-v1" => writer_pre_read_trajectories(&args.run_id),
        "writer-pre-read-v2" => writer_pre_read_v2_trajectories(&args.run_id),
        "writer-pre-read-v3" => writer_pre_read_v3_trajectories(&args.run_id),
        "writer-pre-read-v4" => writer_pre_read_v4_trajectories(&args.run_id),
        "writer-exec-v1" => writer_exec_v1_trajectories(&args.run_id),
        other => {
            return Err(format!(
                "unknown --suite `{other}`; expected `golden`, `golden-v3`, `golden-v4`, `read-generalization`, `read-rare-v1`, `read-api-mcp-v1`, `writer-pre-read-v1`, `writer-pre-read-v2`, `writer-pre-read-v3`, `writer-pre-read-v4`, or `writer-exec-v1`"
            )
            .into());
        }
    };
    validate_trajectories(&trajectories)?;
    write_jsonl(&args.output.join("trajectories.jsonl"), &trajectories)?;
    let summary = summary(&args, &trajectories)?;
    write_json(&args.output.join("summary.json"), &summary)?;

    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}

fn parse_args(
    mut args: impl Iterator<Item = String>,
) -> Result<Args, Box<dyn Error + Send + Sync>> {
    let mut output = None;
    let mut run_id = DEFAULT_RUN_ID.to_string();
    let mut suite = "golden".to_string();
    let mut force = false;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--output" => output = Some(PathBuf::from(next_arg(&mut args, "--output")?)),
            "--run-id" => run_id = next_arg(&mut args, "--run-id")?,
            "--suite" => suite = next_arg(&mut args, "--suite")?,
            "--force" => force = true,
            "--help" | "-h" => return Err(usage().into()),
            value if value.starts_with('-') => {
                return Err(format!("unknown argument: {value}\n{}", usage()).into());
            }
            value => {
                if output.is_some() {
                    return Err(format!("unexpected positional argument: {value}").into());
                }
                output = Some(PathBuf::from(value));
            }
        }
    }

    Ok(Args {
        output: output.ok_or("--output is required")?,
        run_id,
        suite,
        force,
    })
}

fn next_arg(
    args: &mut impl Iterator<Item = String>,
    name: &str,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    args.next()
        .ok_or_else(|| format!("{name} requires a value").into())
}

fn usage() -> String {
    "usage: underpass_operator_conformance_trajectory_build --output <dir> [--run-id id] [--suite golden|golden-v3|golden-v4|read-generalization|read-rare-v1|read-api-mcp-v1|writer-pre-read-v1|writer-pre-read-v2|writer-pre-read-v3|writer-pre-read-v4|writer-exec-v1] [--force]"
        .to_string()
}

fn ensure_output_dir(path: &Path, force: bool) -> Result<(), Box<dyn Error + Send + Sync>> {
    if path.exists() {
        if !force {
            return Err(format!(
                "output directory already exists: {}; pass --force to replace generated files",
                path.display()
            )
            .into());
        }
    } else {
        fs::create_dir_all(path)?;
    }
    Ok(())
}

fn conformance_trajectories(run_id: &str) -> Vec<TrajectoryItem> {
    let mut items = Vec::new();
    let read_tools = kernel_operator_allowed_read_tools();
    let full_tools = kernel_operator_allowed_full_tools();
    let about = "incident:mobile-login";
    let sibling_about = "incident:payments";
    let current_ref = "incident:mobile-login:question:login-failure";
    let prior_observation_ref = "incident:mobile-login:observation:401-refresh-race";
    let prior_timeout_ref = "incident:mobile-login:hypothesis:network-timeout";
    let final_decision_ref = "incident:mobile-login:decision:refresh-retry";
    let stale_decision_ref = "incident:mobile-login:decision:widen-timeout";

    push(
        &mut items,
        item(
            run_id,
            "conformance.read.wake",
            "read",
            about,
            "wake-current-about",
            "Wake the current about before deciding a bounded navigation move.",
            json!({
                "current_ref": current_ref,
                "known_refs": [],
                "last_tool": null,
                "remaining_budget": budget(5),
                "requested_wake": requested_wake(
                    "resume_process",
                    json!({ "mode": "all", "scope": "current_about" }),
                    2400,
                    2
                ),
                "operator_state": {
                    "decision": "wake_current_about",
                    "why": "No memory refs are visible yet."
                }
            }),
            read_tools.clone(),
            tool_call(
                "kernel_wake",
                json!({
                    "about": about,
                    "role": "operator",
                    "intent": "resume_process",
                    "dimensions": { "mode": "all", "scope": "current_about" },
                    "depth": 2,
                    "budget": { "tokens": 2400, "depth": 2 }
                }),
            ),
            json!({ "success": true, "observed_refs": [current_ref, prior_observation_ref] }),
            json!({ "bounded": true, "contract_expected": true }),
        ),
    );

    push(
        &mut items,
        item(
            run_id,
            "conformance.read.wake",
            "read",
            about,
            "wake-after-context-reset",
            "Wake the current about after a reset when no candidate refs survived.",
            json!({
                "current_ref": null,
                "known_refs": [],
                "last_tool": "kernel_near",
                "last_observed_refs": [],
                "last_result_count": 0,
                "remaining_budget": budget(4),
                "requested_wake": requested_wake(
                    "recover_after_empty_navigation",
                    json!({ "mode": "all", "scope": "current_about" }),
                    2400,
                    2
                ),
                "operator_state": {
                    "decision": "wake_current_about",
                    "why": "The prior bounded read produced no usable refs after reset."
                }
            }),
            read_tools.clone(),
            tool_call(
                "kernel_wake",
                json!({
                    "about": about,
                    "role": "operator",
                    "intent": "recover_after_empty_navigation",
                    "dimensions": { "mode": "all", "scope": "current_about" },
                    "depth": 2,
                    "budget": { "tokens": 2400, "depth": 2 }
                }),
            ),
            json!({ "success": true, "observed_refs": [current_ref, prior_timeout_ref] }),
            json!({ "bounded": true, "contract_expected": true, "variant": "wake_after_reset" }),
        ),
    );

    push(
        &mut items,
        item(
            run_id,
            "conformance.read.ask",
            "read",
            about,
            "ask-current-about",
            "Ask the current about: What evidence explains the failed mobile login?",
            json!({
                "current_ref": current_ref,
                "known_refs": [current_ref],
                "last_tool": "kernel_wake",
                "last_observed_refs": [current_ref, prior_observation_ref],
                "remaining_budget": budget(4),
                "requested_ask": requested_ask(
                    "What evidence explains the failed mobile login?",
                    "evidence_or_unknown",
                    json!({ "mode": "all", "scope": "current_about" }),
                    2400,
                    2
                ),
                "operator_state": {
                    "decision": "ask_for_context",
                    "answer_policy": "evidence_or_unknown"
                }
            }),
            read_tools.clone(),
            tool_call(
                "kernel_ask",
                json!({
                    "about": about,
                    "answer_policy": "evidence_or_unknown",
                    "dimensions": { "mode": "all", "scope": "current_about" },
                    "question": "What evidence explains the failed mobile login?",
                    "budget": { "tokens": 2400 },
                    "depth": 2
                }),
            ),
            json!({ "success": true, "observed_refs": [prior_observation_ref] }),
            json!({ "bounded": true, "contract_expected": true }),
        ),
    );

    push(
        &mut items,
        item(
            run_id,
            "conformance.read.ask",
            "read",
            about,
            "ask-conflicts-only-dimensions",
            "Ask for conflicts in agent:triage and agent:solver dimensions: Which hypotheses conflict with the observed refresh race?",
            json!({
                "current_ref": current_ref,
                "known_refs": [current_ref, prior_timeout_ref, prior_observation_ref],
                "last_tool": "kernel_ask",
                "last_observed_refs": [prior_timeout_ref, prior_observation_ref],
                "remaining_budget": budget(4),
                "requested_ask": requested_ask(
                    "Which hypotheses conflict with the observed refresh race?",
                    "show_conflicts",
                    json!({
                        "mode": "only",
                        "scope": "current_about",
                        "include": ["agent:triage", "agent:solver"]
                    }),
                    2400,
                    2
                ),
                "operator_state": {
                    "decision": "ask_conflicts",
                    "needed_dimensions": ["agent:triage", "agent:solver"]
                }
            }),
            read_tools.clone(),
            tool_call(
                "kernel_ask",
                json!({
                    "about": about,
                    "answer_policy": "show_conflicts",
                    "dimensions": {
                        "mode": "only",
                        "scope": "current_about",
                        "include": ["agent:triage", "agent:solver"]
                    },
                    "question": "Which hypotheses conflict with the observed refresh race?",
                    "budget": { "tokens": 2400 },
                    "depth": 2
                }),
            ),
            json!({ "success": true, "observed_refs": [prior_timeout_ref, prior_observation_ref] }),
            json!({ "bounded": true, "contract_expected": true }),
        ),
    );

    push(
        &mut items,
        item(
            run_id,
            "conformance.read.ask",
            "read",
            about,
            "ask-all-abouts",
            "Ask all abouts explicitly: Did a similar refresh race happen in another incident?",
            json!({
                "current_ref": current_ref,
                "known_refs": [current_ref],
                "last_tool": "kernel_ask",
                "last_observed_refs": [],
                "remaining_budget": budget(3),
                "requested_ask": requested_ask(
                    "Did a similar refresh race happen in another incident?",
                    "evidence_or_unknown",
                    json!({ "mode": "all", "scope": "all_abouts" }),
                    3200,
                    2
                ),
                "operator_state": {
                    "decision": "use_all_abouts_explicitly",
                    "why": "The question compares incidents across products."
                }
            }),
            read_tools.clone(),
            tool_call(
                "kernel_ask",
                json!({
                    "about": about,
                    "answer_policy": "evidence_or_unknown",
                    "dimensions": { "mode": "all", "scope": "all_abouts" },
                    "question": "Did a similar refresh race happen in another incident?",
                    "budget": { "tokens": 3200 },
                    "depth": 2
                }),
            ),
            json!({ "success": true, "observed_refs": ["incident:payments:observation:token-refresh-race"] }),
            json!({ "bounded": true, "contract_expected": true, "scope_is_intentional": true }),
        ),
    );

    push(
        &mut items,
        item(
            run_id,
            "conformance.read.ask",
            "read",
            about,
            "ask-all-abouts-after-sibling-signal",
            "Ask all abouts explicitly after a sibling incident hint appears.",
            json!({
                "current_ref": current_ref,
                "known_refs": [current_ref, "incident:payments:hint:auth-regression"],
                "last_tool": "kernel_near",
                "last_observed_refs": ["incident:payments:hint:auth-regression"],
                "remaining_budget": budget(3),
                "requested_ask": requested_ask(
                    "Does another incident show the same auth regression pattern?",
                    "evidence_or_unknown",
                    json!({ "mode": "all", "scope": "all_abouts" }),
                    3200,
                    2
                ),
                "operator_state": {
                    "decision": "use_all_abouts_explicitly",
                    "why": "A visible sibling hint makes the cross-incident lookup intentional."
                }
            }),
            read_tools.clone(),
            tool_call(
                "kernel_ask",
                json!({
                    "about": about,
                    "answer_policy": "evidence_or_unknown",
                    "dimensions": { "mode": "all", "scope": "all_abouts" },
                    "question": "Does another incident show the same auth regression pattern?",
                    "budget": { "tokens": 3200 },
                    "depth": 2
                }),
            ),
            json!({ "success": true, "observed_refs": ["incident:payments:observation:token-refresh-race"] }),
            json!({ "bounded": true, "contract_expected": true, "scope_is_intentional": true, "variant": "sibling_signal" }),
        ),
    );

    push(
        &mut items,
        item(
            run_id,
            "conformance.read.near",
            "read",
            about,
            "near-by-ref-shrink-window-except-discarded",
            "Read near the current ref with a smaller window and excluded discarded dimensions.",
            json!({
                "current_ref": current_ref,
                "known_refs": [current_ref],
                "last_tool": "kernel_ask",
                "last_observed_refs": [current_ref],
                "remaining_budget": budget(4),
                "requested_move": requested_move(
                    "kernel_near",
                    "around",
                    json!({ "ref": current_ref })
                ),
                "requested_scope": json!({ "mode": "except", "scope": "current_about", "exclude": ["attempt:discarded"] }),
                "requested_bounds": requested_bounds(
                    json!({ "entries": 6, "tokens": 1600 }),
                    json!({ "before_entries": 3, "after_entries": 0 })
                ),
                "operator_state": {
                    "decision": "shrink_window",
                    "why": "The candidate is already precise."
                }
            }),
            read_tools.clone(),
            temporal_call(
                "kernel_near",
                "around",
                json!({ "ref": current_ref }),
                json!({ "mode": "except", "scope": "current_about", "exclude": ["attempt:discarded"] }),
                json!({ "entries": 6, "tokens": 1600 }),
                json!({ "before_entries": 3, "after_entries": 0 }),
            ),
            json!({ "success": true, "observed_refs": [prior_observation_ref] }),
            json!({ "bounded": true, "contract_expected": true, "policy": "shrink_window" }),
        ),
    );

    push(
        &mut items,
        item(
            run_id,
            "conformance.read.near",
            "read",
            about,
            "near-by-time-expand-window-about-scope",
            "Read near a timestamp across an explicit about list with a larger window.",
            json!({
                "current_ref": current_ref,
                "known_refs": [current_ref],
                "last_tool": "kernel_near",
                "last_observed_refs": [prior_observation_ref],
                "requested_move": requested_move(
                    "kernel_near",
                    "around",
                    json!({ "time": "2026-05-06T10:04:00Z" })
                ),
                "requested_scope": {
                    "mode": "all",
                    "scope": "abouts",
                    "abouts": [about, sibling_about]
                },
                "requested_bounds": requested_bounds(
                    json!({ "entries": 24, "tokens": 3200 }),
                    json!({ "before_entries": 12, "after_entries": 6 })
                ),
                "remaining_budget": budget(4),
                "operator_state": {
                    "decision": "expand_window",
                    "why": "The previous read returned too few refs."
                }
            }),
            read_tools.clone(),
            temporal_call(
                "kernel_near",
                "around",
                json!({ "time": "2026-05-06T10:04:00Z" }),
                json!({
                    "mode": "all",
                    "scope": "abouts",
                    "abouts": [about, sibling_about]
                }),
                json!({ "entries": 24, "tokens": 3200 }),
                json!({ "before_entries": 12, "after_entries": 6 }),
            ),
            json!({ "success": true, "observed_refs": [prior_observation_ref, "incident:payments:observation:token-refresh-race"] }),
            json!({ "bounded": true, "contract_expected": true, "policy": "expand_window" }),
        ),
    );

    push_temporal_tool(
        &mut items,
        run_id,
        "kernel_goto",
        "at",
        json!({ "ref": prior_observation_ref }),
        "goto-by-ref",
        "Move directly to a known observation ref for reproducible replay.",
        read_tools.clone(),
    );
    push_temporal_tool(
        &mut items,
        run_id,
        "kernel_goto",
        "at",
        json!({ "time": "2026-05-06T10:04:00Z" }),
        "goto-by-time",
        "Move to the memory state known at an explicit timestamp.",
        read_tools.clone(),
    );
    push_temporal_tool(
        &mut items,
        run_id,
        "kernel_goto",
        "at",
        json!({ "sequence": 7 }),
        "goto-by-sequence",
        "Move to the known sequence coordinate for reproducible replay.",
        read_tools.clone(),
    );
    push_temporal_tool(
        &mut items,
        run_id,
        "kernel_rewind",
        "from",
        json!({ "ref": final_decision_ref }),
        "rewind-from-decision-ref",
        "Move backward from the final decision to find the prior cause.",
        read_tools.clone(),
    );
    push_temporal_tool(
        &mut items,
        run_id,
        "kernel_rewind",
        "from",
        json!({ "time": "2026-05-06T10:05:00Z" }),
        "rewind-from-time",
        "Move backward from a timestamp to find the previous relevant decision.",
        read_tools.clone(),
    );
    push_temporal_tool(
        &mut items,
        run_id,
        "kernel_rewind",
        "from",
        json!({ "sequence": 9 }),
        "rewind-from-sequence",
        "Move backward from a sequence coordinate to find prior evidence.",
        read_tools.clone(),
    );
    push_temporal_tool(
        &mut items,
        run_id,
        "kernel_forward",
        "from",
        json!({ "ref": prior_observation_ref }),
        "forward-from-ref",
        "Move forward from the observation to find the later decision.",
        read_tools.clone(),
    );
    push_temporal_tool(
        &mut items,
        run_id,
        "kernel_forward",
        "from",
        json!({ "time": "2026-05-06T10:00:00Z" }),
        "forward-from-time",
        "Move forward from the observed time to find the later update.",
        read_tools.clone(),
    );
    push_temporal_tool(
        &mut items,
        run_id,
        "kernel_forward",
        "from",
        json!({ "sequence": 4 }),
        "forward-from-sequence",
        "Move forward from a sequence coordinate to find the next update.",
        read_tools.clone(),
    );

    push(
        &mut items,
        item(
            run_id,
            "conformance.read.near",
            "read",
            about,
            "near-by-ref-expand-after-empty",
            "Expand the nearby window after the previous bounded read found no evidence.",
            json!({
                "current_ref": current_ref,
                "known_refs": [current_ref],
                "last_tool": "kernel_near",
                "last_observed_refs": [],
                "last_result_count": 0,
                "last_result_partial": false,
                "remaining_budget": budget(4),
                "requested_move": requested_move(
                    "kernel_near",
                    "around",
                    json!({ "ref": current_ref })
                ),
                "requested_scope": json!({ "mode": "all", "scope": "current_about" }),
                "requested_bounds": requested_bounds(
                    json!({ "entries": 32, "tokens": 3600 }),
                    json!({ "before_entries": 16, "after_entries": 8 })
                ),
                "operator_state": {
                    "decision": "expand_window",
                    "why": "The first nearby read was too narrow and returned no refs."
                }
            }),
            read_tools.clone(),
            temporal_call(
                "kernel_near",
                "around",
                json!({ "ref": current_ref }),
                json!({ "mode": "all", "scope": "current_about" }),
                json!({ "entries": 32, "tokens": 3600 }),
                json!({ "before_entries": 16, "after_entries": 8 }),
            ),
            json!({ "success": true, "observed_refs": [prior_timeout_ref, prior_observation_ref] }),
            json!({ "bounded": true, "contract_expected": true, "policy": "expand_window" }),
        ),
    );

    push(
        &mut items,
        item(
            run_id,
            "conformance.read.near",
            "read",
            about,
            "near-by-sequence-shrink-after-precise-hit",
            "Shrink the nearby window when a prior read already identified the relevant sequence.",
            json!({
                "current_ref": final_decision_ref,
                "known_refs": [final_decision_ref, prior_observation_ref],
                "last_tool": "kernel_forward",
                "last_observed_refs": [final_decision_ref],
                "last_result_count": 1,
                "requested_move": requested_move(
                    "kernel_near",
                    "around",
                    json!({ "sequence": 9 })
                ),
                "requested_scope": json!({ "mode": "all", "scope": "current_about" }),
                "requested_bounds": requested_bounds(
                    json!({ "entries": 4, "tokens": 1200 }),
                    json!({ "before_entries": 2, "after_entries": 1 })
                ),
                "remaining_budget": budget(2),
                "operator_state": {
                    "decision": "shrink_window",
                    "why": "The target sequence is precise and token budget is low."
                }
            }),
            read_tools.clone(),
            temporal_call(
                "kernel_near",
                "around",
                json!({ "sequence": 9 }),
                json!({ "mode": "all", "scope": "current_about" }),
                json!({ "entries": 4, "tokens": 1200 }),
                json!({ "before_entries": 2, "after_entries": 1 }),
            ),
            json!({ "success": true, "observed_refs": [final_decision_ref] }),
            json!({ "bounded": true, "contract_expected": true, "policy": "shrink_window" }),
        ),
    );

    push(
        &mut items,
        item(
            run_id,
            "conformance.read.trace",
            "read",
            about,
            "trace-first-page",
            "Trace a path using an explicit first page limit.",
            json!({
                "current_ref": prior_observation_ref,
                "trace_target_ref": final_decision_ref,
                "known_refs": [
                    "incident:mobile-login:entry:observation:refresh-race-confirmed",
                    prior_observation_ref,
                    stale_decision_ref,
                    final_decision_ref
                ],
                "last_tool": "kernel_forward",
                "last_observed_refs": [final_decision_ref],
                "last_result_page": null,
                "remaining_budget": budget(3),
                "requested_trace": requested_trace(
                    prior_observation_ref,
                    final_decision_ref,
                    "Trace why the refresh retry decision was chosen.",
                    json!({ "entries": 16 })
                ),
                "operator_state": { "decision": "trace_first_page" }
            }),
            read_tools.clone(),
            tool_call(
                "kernel_trace",
                json!({
                    "from": prior_observation_ref,
                    "to": final_decision_ref,
                    "goal": "Trace why the refresh retry decision was chosen.",
                    "role": "operator",
                    "budget": { "depth": 2, "tokens": 2400 },
                    "page": { "entries": 16 }
                }),
            ),
            json!({
                "success": true,
                "observed_refs": [prior_observation_ref, final_decision_ref],
                "page": { "entries": 16, "has_more": true, "next_cursor": "16" }
            }),
            json!({ "bounded": true, "contract_expected": true, "policy": "trace_first_page" }),
        ),
    );

    push(
        &mut items,
        item(
            run_id,
            "conformance.read.trace",
            "read",
            about,
            "trace-first-page-after-new-target",
            "Start a new trace first page after the target changes.",
            json!({
                "current_ref": prior_timeout_ref,
                "trace_target_ref": final_decision_ref,
                "known_refs": [
                    prior_timeout_ref,
                    prior_observation_ref,
                    final_decision_ref
                ],
                "last_tool": "kernel_near",
                "last_observed_refs": [prior_timeout_ref, prior_observation_ref],
                "last_result_page": null,
                "remaining_budget": budget(3),
                "requested_trace": requested_trace(
                    prior_timeout_ref,
                    final_decision_ref,
                    "Trace why the timeout hypothesis was superseded.",
                    json!({ "entries": 16 })
                ),
                "operator_state": { "decision": "trace_first_page" }
            }),
            read_tools.clone(),
            tool_call(
                "kernel_trace",
                json!({
                    "from": prior_timeout_ref,
                    "to": final_decision_ref,
                    "goal": "Trace why the timeout hypothesis was superseded.",
                    "role": "operator",
                    "budget": { "depth": 2, "tokens": 2400 },
                    "page": { "entries": 16 }
                }),
            ),
            json!({
                "success": true,
                "observed_refs": [prior_timeout_ref, prior_observation_ref, final_decision_ref],
                "page": { "entries": 16, "has_more": false, "next_cursor": null }
            }),
            json!({ "bounded": true, "contract_expected": true, "policy": "trace_first_page", "variant": "new_target" }),
        ),
    );

    push(
        &mut items,
        item(
            run_id,
            "conformance.read.trace",
            "read",
            about,
            "trace-continue-page",
            "Continue a partial trace using the returned page cursor.",
            json!({
                "current_ref": prior_observation_ref,
                "trace_target_ref": final_decision_ref,
                "known_refs": [
                    "incident:mobile-login:entry:observation:refresh-race-confirmed",
                    prior_observation_ref,
                    stale_decision_ref,
                    final_decision_ref
                ],
                "last_tool": "kernel_trace",
                "last_observed_refs": [prior_observation_ref, final_decision_ref],
                "last_result_page": {
                    "entries": 16,
                    "has_more": true,
                    "next_cursor": "16"
                },
                "last_result_partial": true,
                "remaining_budget": budget(2),
                "requested_trace": requested_trace(
                    prior_observation_ref,
                    final_decision_ref,
                    "Continue the partial trace.",
                    json!({ "entries": 16, "cursor": "16" })
                ),
                "operator_state": { "decision": "continue_page" }
            }),
            read_tools.clone(),
            tool_call(
                "kernel_trace",
                json!({
                    "from": prior_observation_ref,
                    "to": final_decision_ref,
                    "goal": "Continue the partial trace.",
                    "role": "operator",
                    "budget": { "depth": 2, "tokens": 2400 },
                    "page": { "entries": 16, "cursor": "16" }
                }),
            ),
            json!({
                "success": true,
                "observed_refs": [prior_observation_ref, stale_decision_ref, final_decision_ref],
                "page": { "entries": 16, "has_more": false }
            }),
            json!({ "bounded": true, "contract_expected": true, "policy": "continue_page" }),
        ),
    );

    push(
        &mut items,
        item(
            run_id,
            "conformance.read.trace",
            "read",
            about,
            "trace-continue-third-page",
            "Continue a longer trace with the second returned cursor.",
            json!({
                "current_ref": prior_observation_ref,
                "trace_target_ref": final_decision_ref,
                "known_refs": [prior_observation_ref, stale_decision_ref, final_decision_ref],
                "last_tool": "kernel_trace",
                "last_observed_refs": [prior_observation_ref, stale_decision_ref],
                "last_result_page": {
                    "entries": 16,
                    "has_more": true,
                    "next_cursor": "32"
                },
                "last_result_partial": true,
                "remaining_budget": budget(2),
                "requested_trace": requested_trace(
                    prior_observation_ref,
                    final_decision_ref,
                    "Continue the longer partial trace.",
                    json!({ "entries": 16, "cursor": "32" })
                ),
                "operator_state": { "decision": "continue_page" }
            }),
            read_tools.clone(),
            tool_call(
                "kernel_trace",
                json!({
                    "from": prior_observation_ref,
                    "to": final_decision_ref,
                    "goal": "Continue the longer partial trace.",
                    "role": "operator",
                    "budget": { "depth": 2, "tokens": 2400 },
                    "page": { "entries": 16, "cursor": "32" }
                }),
            ),
            json!({
                "success": true,
                "observed_refs": [prior_observation_ref, stale_decision_ref, final_decision_ref],
                "page": { "entries": 16, "has_more": false }
            }),
            json!({ "bounded": true, "contract_expected": true, "policy": "continue_page" }),
        ),
    );

    push(
        &mut items,
        item(
            run_id,
            "conformance.read.inspect",
            "read",
            about,
            "inspect-typed-raw-false",
            "Inspect a typed node without raw memory expansion.",
            json!({
                "current_ref": prior_observation_ref,
                "known_refs": [prior_observation_ref],
                "last_tool": "kernel_near",
                "last_observed_refs": [prior_observation_ref],
                "remaining_budget": budget(2),
                "inspection_request": inspection_request(prior_observation_ref),
                "operator_state": {
                    "decision": "inspect_typed",
                    "raw_allowed": false
                }
            }),
            read_tools.clone(),
            tool_call(
                "kernel_inspect",
                json!({
                    "ref": prior_observation_ref,
                    "include": {
                        "details": true,
                        "incoming": true,
                        "outgoing": true,
                        "raw": false
                    }
                }),
            ),
            json!({ "success": true, "observed_refs": [prior_observation_ref] }),
            json!({ "bounded": true, "contract_expected": true, "raw_access": false }),
        ),
    );

    push(
        &mut items,
        item(
            run_id,
            "conformance.read.inspect",
            "read",
            about,
            "inspect-final-decision-raw-false",
            "Inspect the visible final decision without raw memory expansion.",
            json!({
                "current_ref": final_decision_ref,
                "known_refs": [final_decision_ref, prior_observation_ref],
                "last_tool": "kernel_trace",
                "last_observed_refs": [final_decision_ref, prior_observation_ref],
                "remaining_budget": budget(2),
                "inspection_request": inspection_request(final_decision_ref),
                "operator_state": {
                    "decision": "inspect_typed",
                    "raw_allowed": false
                }
            }),
            read_tools.clone(),
            tool_call(
                "kernel_inspect",
                json!({
                    "ref": final_decision_ref,
                    "include": {
                        "details": true,
                        "incoming": true,
                        "outgoing": true,
                        "raw": false
                    }
                }),
            ),
            json!({ "success": true, "observed_refs": [final_decision_ref] }),
            json!({ "bounded": true, "contract_expected": true, "raw_access": false, "variant": "final_decision" }),
        ),
    );

    push(
        &mut items,
        item(
            run_id,
            "conformance.read.inspect",
            "read",
            about,
            "inspect-timeout-hypothesis-raw-false",
            "Inspect the visible timeout hypothesis without raw memory expansion.",
            json!({
                "current_ref": prior_timeout_ref,
                "known_refs": [prior_timeout_ref, prior_observation_ref],
                "last_tool": "kernel_rewind",
                "last_observed_refs": [prior_timeout_ref],
                "remaining_budget": budget(2),
                "inspection_request": inspection_request(prior_timeout_ref),
                "operator_state": {
                    "decision": "inspect_typed",
                    "raw_allowed": false
                }
            }),
            read_tools.clone(),
            tool_call(
                "kernel_inspect",
                json!({
                    "ref": prior_timeout_ref,
                    "include": {
                        "details": true,
                        "incoming": true,
                        "outgoing": true,
                        "raw": false
                    }
                }),
            ),
            json!({ "success": true, "observed_refs": [prior_timeout_ref] }),
            json!({ "bounded": true, "contract_expected": true, "raw_access": false, "variant": "timeout_hypothesis" }),
        ),
    );

    push(
        &mut items,
        item(
            run_id,
            "conformance.read.inspect",
            "read",
            about,
            "inspect-stale-decision-raw-false",
            "Inspect the visible stale decision without raw memory expansion.",
            json!({
                "current_ref": stale_decision_ref,
                "known_refs": [stale_decision_ref, final_decision_ref],
                "last_tool": "kernel_trace",
                "last_observed_refs": [stale_decision_ref, final_decision_ref],
                "remaining_budget": budget(2),
                "inspection_request": inspection_request(stale_decision_ref),
                "operator_state": {
                    "decision": "inspect_typed",
                    "raw_allowed": false
                }
            }),
            read_tools.clone(),
            tool_call(
                "kernel_inspect",
                json!({
                    "ref": stale_decision_ref,
                    "include": {
                        "details": true,
                        "incoming": true,
                        "outgoing": true,
                        "raw": false
                    }
                }),
            ),
            json!({ "success": true, "observed_refs": [stale_decision_ref] }),
            json!({ "bounded": true, "contract_expected": true, "raw_access": false, "variant": "stale_decision" }),
        ),
    );

    push(
        &mut items,
        item(
            run_id,
            "conformance.read.stop",
            "read",
            about,
            "stop-sufficient-evidence",
            "Stop once evidence is sufficient and visible.",
            json!({
                "current_ref": current_ref,
                "known_refs": [current_ref, prior_observation_ref, final_decision_ref],
                "last_tool": "kernel_inspect",
                "last_observed_refs": [prior_observation_ref, final_decision_ref],
                "remaining_budget": budget(0),
                "requested_stop": requested_stop(
                    "evidence_or_unknown",
                    vec![prior_observation_ref, final_decision_ref],
                    "sufficient_evidence"
                ),
                "operator_state": {
                    "decision": "stop_sufficient",
                    "evidence_sufficient": true
                }
            }),
            read_tools.clone(),
            stop_action(
                "evidence_or_unknown",
                vec![prior_observation_ref, final_decision_ref],
                "sufficient_evidence",
            ),
            json!({ "success": true, "observed_refs": [prior_observation_ref, final_decision_ref] }),
            json!({ "bounded": true, "contract_expected": true, "stop_correct": true }),
        ),
    );

    push(
        &mut items,
        item(
            run_id,
            "conformance.write.smart",
            "write",
            about,
            "write-memory-rich-relation",
            "Write a smart memory event only after reading the target evidence.",
            json!({
                "current_ref": "incident:mobile-login:draft:decision:refresh-retry",
                "known_refs": [prior_observation_ref, stale_decision_ref],
                "last_tool": "kernel_inspect",
                "last_observed_refs": [prior_observation_ref],
                "candidate_refs": [prior_observation_ref, stale_decision_ref],
                "read_context": {
                    "inspected_refs": [prior_observation_ref],
                    "temporal_refs": [stale_decision_ref]
                },
                "draft_write": {
                    "intent": "record_decision",
                    "prepared_arguments": write_memory_arguments(true),
                    "current": {
                        "kind": "decision",
                        "summary": "Use token refresh retry instead of widening timeout.",
                        "evidence": "Auth logs show 401 immediately after refresh."
                    },
                    "relation_requirement": "rich relation requires why, evidence, and read_context proof"
                },
                "remaining_budget": budget(2),
                "operator_state": {
                    "decision": "write_memory",
                    "relation": "chosen_because",
                    "relation_quality": "rich"
                }
            }),
            full_tools.clone(),
            tool_call("kernel_write_memory", write_memory_arguments(true)),
            json!({
                "success": true,
                "dry_run": true,
                "compiled_to": "kernel_ingest",
                "relation_quality": {
                    "relation_rich_count": 1,
                    "relation_anemic_count": 0,
                    "relation_proof_coverage": 1.0
                }
            }),
            json!({
                "bounded": true,
                "contract_expected": true,
                "write_relation_quality": "rich",
                "read_context_proof": true
            }),
        ),
    );

    push(
        &mut items,
        item(
            run_id,
            "conformance.write.smart",
            "write",
            about,
            "write-memory-rich-without-semantic-delta",
            "Write a rich relation without adding an optional semantic delta.",
            json!({
                "current_ref": "incident:mobile-login:draft:observation:refresh-race-confirmed",
                "known_refs": [prior_observation_ref],
                "last_tool": "kernel_inspect",
                "last_observed_refs": [prior_observation_ref],
                "candidate_refs": [prior_observation_ref],
                "read_context": {
                    "inspected_refs": [prior_observation_ref]
                },
                "draft_write": {
                    "intent": "record_observation",
                    "prepared_arguments": write_memory_rich_without_delta_arguments(),
                    "current": {
                        "kind": "observation",
                        "summary": "Token refresh race confirmed by auth log ordering.",
                        "evidence": "Refresh success is followed by 401 on the next login attempt."
                    },
                    "relation_requirement": "rich relation does not require semantic_delta unless a state change is being recorded"
                },
                "remaining_budget": budget(2),
                "operator_state": {
                    "decision": "write_memory",
                    "relation": "supports",
                    "relation_quality": "rich",
                    "semantic_delta_required": false
                }
            }),
            full_tools.clone(),
            tool_call(
                "kernel_write_memory",
                write_memory_rich_without_delta_arguments(),
            ),
            json!({
                "success": true,
                "dry_run": true,
                "compiled_to": "kernel_ingest",
                "relation_quality": {
                    "relation_rich_count": 1,
                    "relation_anemic_count": 0,
                    "relation_proof_coverage": 1.0
                }
            }),
            json!({
                "bounded": true,
                "contract_expected": true,
                "write_relation_quality": "rich",
                "read_context_proof": true,
                "semantic_delta": false
            }),
        ),
    );

    push(
        &mut items,
        item(
            run_id,
            "conformance.write.smart",
            "write",
            about,
            "write-memory-anemic-fallback",
            "Use an anemic relation when no richer dependency is justified.",
            json!({
                "current_ref": "incident:mobile-login:draft:turn:follow-up",
                "known_refs": [final_decision_ref],
                "last_tool": "kernel_near",
                "last_observed_refs": [final_decision_ref],
                "candidate_refs": [final_decision_ref],
                "read_context": { "temporal_refs": [final_decision_ref] },
                "draft_write": {
                    "intent": "record_turn",
                    "prepared_arguments": write_memory_anemic_arguments(),
                    "current": {
                        "kind": "turn",
                        "summary": "The operator recorded a follow-up status check.",
                        "evidence": "The follow-up only continues the process timeline."
                    },
                    "relation_requirement": "fallback to anemic follows when no richer relation is justified"
                },
                "remaining_budget": budget(2),
                "operator_state": {
                    "decision": "write_memory",
                    "relation": "follows",
                    "relation_quality": "anemic"
                }
            }),
            full_tools.clone(),
            tool_call("kernel_write_memory", write_memory_anemic_arguments()),
            json!({
                "success": true,
                "dry_run": true,
                "compiled_to": "kernel_ingest",
                "relation_quality": {
                    "relation_rich_count": 0,
                    "relation_anemic_count": 1,
                    "relation_proof_coverage": 1.0
                }
            }),
            json!({
                "bounded": true,
                "contract_expected": true,
                "write_relation_quality": "anemic",
                "read_context_proof": true
            }),
        ),
    );

    push(
        &mut items,
        item(
            run_id,
            "conformance.write.smart",
            "write",
            about,
            "write-memory-anemic-without-semantic-delta",
            "Write an anemic process relation without inventing semantic delta.",
            json!({
                "current_ref": "incident:mobile-login:draft:turn:operator-note",
                "known_refs": [final_decision_ref],
                "last_tool": "kernel_near",
                "last_observed_refs": [final_decision_ref],
                "candidate_refs": [final_decision_ref],
                "read_context": { "temporal_refs": [final_decision_ref] },
                "draft_write": {
                    "intent": "record_turn",
                    "prepared_arguments": write_memory_anemic_without_delta_arguments(),
                    "current": {
                        "kind": "turn",
                        "summary": "Operator added a short follow-up note.",
                        "evidence": "The note only follows the prior decision in process order."
                    },
                    "relation_requirement": "anemic follows relation; no semantic_delta is justified"
                },
                "remaining_budget": budget(2),
                "operator_state": {
                    "decision": "write_memory",
                    "relation": "follows",
                    "relation_quality": "anemic",
                    "semantic_delta_required": false
                }
            }),
            full_tools.clone(),
            tool_call(
                "kernel_write_memory",
                write_memory_anemic_without_delta_arguments(),
            ),
            json!({
                "success": true,
                "dry_run": true,
                "compiled_to": "kernel_ingest",
                "relation_quality": {
                    "relation_rich_count": 0,
                    "relation_anemic_count": 1,
                    "relation_proof_coverage": 1.0
                }
            }),
            json!({
                "bounded": true,
                "contract_expected": true,
                "write_relation_quality": "anemic",
                "read_context_proof": true,
                "semantic_delta": false
            }),
        ),
    );

    push(
        &mut items,
        item(
            run_id,
            "conformance.write.failfast",
            "write",
            about,
            "write-memory-stop-without-read-context-proof",
            "Fail fast instead of writing a rich relation without read-context proof.",
            json!({
                "current_ref": "incident:mobile-login:draft:decision:unsafe-rich-write",
                "known_refs": [],
                "last_tool": null,
                "last_observed_refs": [],
                "candidate_refs": [prior_observation_ref],
                "read_context": { "inspected_refs": [] },
                "draft_write": {
                    "intent": "record_decision",
                    "current": {
                        "kind": "decision",
                        "summary": "Use token refresh retry.",
                        "evidence": "The draft claims evidence but has not inspected the target."
                    },
                    "proposed_relation": {
                        "ref": prior_observation_ref,
                        "rel": "chosen_because",
                        "class": "causal"
                    }
                },
                "remaining_budget": budget(2),
                "operator_state": {
                    "decision": "stop_missing_read_context_proof",
                    "why": "A rich relation target is not proven visible."
                }
            }),
            full_tools.clone(),
            stop_action(
                "evidence_or_unknown",
                Vec::new(),
                "write_requires_read_context_proof",
            ),
            json!({
                "success": true,
                "write_attempted": false,
                "failfast": true
            }),
            json!({
                "bounded": true,
                "contract_expected": true,
                "expected_failfast": true,
                "invalid_tool_call": false
            }),
        ),
    );

    push(
        &mut items,
        item(
            run_id,
            "conformance.write.failfast",
            "write",
            about,
            "write-memory-stop-ambiguous-relation",
            "Fail fast when the writer cannot honestly choose a relation.",
            json!({
                "current_ref": "incident:mobile-login:draft:decision:ambiguous",
                "known_refs": [prior_observation_ref, stale_decision_ref],
                "last_tool": "kernel_near",
                "last_observed_refs": [prior_observation_ref, stale_decision_ref],
                "candidate_refs": [prior_observation_ref, stale_decision_ref],
                "read_context": {
                    "temporal_refs": [prior_observation_ref, stale_decision_ref]
                },
                "draft_write": {
                    "intent": "record_decision",
                    "current": {
                        "kind": "decision",
                        "summary": "Choose a remediation path.",
                        "evidence": "Both candidates are plausible but neither is proven decisive."
                    },
                    "relation_requirement": "do not invent chosen_because when evidence is ambiguous"
                },
                "remaining_budget": budget(1),
                "operator_state": {
                    "decision": "stop_ambiguous_relation",
                    "why": "A vague rich relation would be misleading."
                }
            }),
            full_tools.clone(),
            stop_action(
                "evidence_or_unknown",
                vec![prior_observation_ref, stale_decision_ref],
                "relation_not_justified",
            ),
            json!({
                "success": true,
                "write_attempted": false,
                "failfast": true
            }),
            json!({
                "bounded": true,
                "contract_expected": true,
                "expected_failfast": true,
                "invalid_tool_call": false
            }),
        ),
    );

    push(
        &mut items,
        item(
            run_id,
            "conformance.write.ingest",
            "write",
            about,
            "ingest-canonical-memory",
            "Use canonical ingest when the writer already has a complete memory payload.",
            json!({
                "current_ref": "incident:mobile-login:entry:decision:refresh-retry",
                "known_refs": [prior_observation_ref],
                "last_tool": "kernel_write_memory",
                "last_observed_refs": [prior_observation_ref],
                "canonical_payload_ready": true,
                "canonical_payload": ingest_arguments(),
                "remaining_budget": budget(1),
                "operator_state": {
                    "decision": "canonical_ingest",
                    "why": "The complete memory graph payload is already typed."
                }
            }),
            full_tools.clone(),
            tool_call("kernel_ingest", ingest_arguments()),
            json!({
                "success": true,
                "dry_run": true,
                "dimensions": 1,
                "entries": 1,
                "relations": 1,
                "evidence": 1
            }),
            json!({
                "bounded": true,
                "contract_expected": true,
                "canonical_write": true
            }),
        ),
    );

    push(
        &mut items,
        item(
            run_id,
            "conformance.write.ingest",
            "write",
            about,
            "ingest-canonical-memory-multi-entry",
            "Use canonical ingest for a complete multi-entry typed payload.",
            json!({
                "current_ref": "incident:mobile-login:entry:decision:final-remediation",
                "known_refs": [
                    "incident:mobile-login:entry:observation:refresh-race-confirmed",
                    prior_observation_ref,
                    stale_decision_ref,
                    final_decision_ref
                ],
                "last_tool": "kernel_write_memory",
                "last_observed_refs": [prior_observation_ref, final_decision_ref],
                "canonical_payload_ready": true,
                "canonical_payload": ingest_multi_entry_arguments(),
                "remaining_budget": budget(1),
                "operator_state": {
                    "decision": "canonical_ingest",
                    "why": "The writer has typed entries, relations, coordinates, and evidence."
                }
            }),
            full_tools.clone(),
            tool_call("kernel_ingest", ingest_multi_entry_arguments()),
            json!({
                "success": true,
                "dry_run": true,
                "dimensions": 2,
                "entries": 2,
                "relations": 2,
                "evidence": 2
            }),
            json!({
                "bounded": true,
                "contract_expected": true,
                "canonical_write": true
            }),
        ),
    );

    push_training_corpus_variants(&mut items, run_id, &read_tools, &full_tools);

    items
}

fn golden_v3_trajectories(run_id: &str) -> Vec<TrajectoryItem> {
    let mut items = conformance_trajectories(run_id);
    let read_tools = kernel_operator_allowed_read_tools();
    let about = "incident:mobile-login";
    let current_ref = "incident:mobile-login:question:login-failure";
    let prior_observation_ref = "incident:mobile-login:observation:401-refresh-race";
    let prior_timeout_ref = "incident:mobile-login:hypothesis:network-timeout";

    push(
        &mut items,
        item(
            run_id,
            "conformance.read.wake",
            "read",
            about,
            "wake-after-empty-near-current-ref-visible",
            "Wake after an empty nearby read even though current_ref is still visible.",
            json!({
                "current_ref": current_ref,
                "known_refs": [],
                "last_tool": "kernel_near",
                "last_observed_refs": [],
                "last_result_count": 0,
                "remaining_budget": budget(4),
                "requested_wake": requested_wake(
                    "recover_after_empty_navigation",
                    json!({ "mode": "all", "scope": "current_about" }),
                    2400,
                    2
                ),
                "operator_state": {
                    "decision": "wake_current_about",
                    "why": "The requested operation is wake; the visible current_ref must not turn it into near."
                }
            }),
            read_tools,
            tool_call(
                "kernel_wake",
                json!({
                    "about": about,
                    "role": "operator",
                    "intent": "recover_after_empty_navigation",
                    "dimensions": { "mode": "all", "scope": "current_about" },
                    "depth": 2,
                    "budget": { "tokens": 2400, "depth": 2 }
                }),
            ),
            json!({ "success": true, "observed_refs": [prior_observation_ref, prior_timeout_ref] }),
            json!({
                "bounded": true,
                "contract_expected": true,
                "variant": "wake_after_empty_near_current_ref_visible"
            }),
        ),
    );

    items
}

fn golden_v4_trajectories(run_id: &str) -> Vec<TrajectoryItem> {
    let mut items = golden_v3_trajectories(run_id);
    let read_tools = kernel_operator_allowed_read_tools();
    let about = "incident:mobile-login";
    let current_ref = "incident:mobile-login:question:login-failure";
    let prior_observation_ref = "incident:mobile-login:observation:401-refresh-race";
    let prior_timeout_ref = "incident:mobile-login:hypothesis:network-timeout";
    let final_decision_ref = "incident:mobile-login:decision:refresh-retry";

    push(
        &mut items,
        item(
            run_id,
            "conformance.read.near",
            "read",
            about,
            "near-by-ref-except-discarded-and-scratch-training-contrast",
            "Read near a visible ref while excluding discarded and scratch dimensions.",
            json!({
                "current_ref": current_ref,
                "known_refs": [current_ref, prior_observation_ref, prior_timeout_ref, final_decision_ref],
                "last_tool": "kernel_ask",
                "last_observed_refs": [prior_observation_ref],
                "last_result_count": 1,
                "last_result_partial": false,
                "remaining_budget": budget(3),
                "requested_move": requested_move(
                    "kernel_near",
                    "around",
                    json!({ "ref": prior_observation_ref })
                ),
                "requested_scope": json!({
                    "mode": "except",
                    "scope": "current_about",
                    "exclude": ["attempt:discarded", "scratch"]
                }),
                "requested_bounds": requested_bounds(
                    json!({ "entries": 7, "tokens": 1700 }),
                    json!({ "before_entries": 2, "after_entries": 1 })
                ),
                "operator_state": {
                    "decision": "near_with_except_dimensions",
                    "why": "Dimension filters must stay under arguments.dimensions."
                }
            }),
            read_tools,
            temporal_call(
                "kernel_near",
                "around",
                json!({ "ref": prior_observation_ref }),
                json!({
                    "mode": "except",
                    "scope": "current_about",
                    "exclude": ["attempt:discarded", "scratch"]
                }),
                json!({ "entries": 7, "tokens": 1700 }),
                json!({ "before_entries": 2, "after_entries": 1 }),
            ),
            json!({ "success": true, "observed_refs": [prior_observation_ref, final_decision_ref] }),
            json!({
                "bounded": true,
                "contract_expected": true,
                "variant": "except_dimension_training_contrast"
            }),
        ),
    );

    items
}

fn read_generalization_trajectories(run_id: &str) -> Vec<TrajectoryItem> {
    let mut items = Vec::new();
    let read_tools = kernel_operator_allowed_read_tools();
    let about = "incident:checkout-latency";
    let sibling_about = "incident:inventory-sync";
    let current_ref = "incident:checkout-latency:question:cart-submit-delay";
    let observation_ref = "incident:checkout-latency:observation:gateway-p99-spike";
    let stale_hypothesis_ref = "incident:checkout-latency:hypothesis:frontend-bundle-size";
    let decision_ref = "incident:checkout-latency:decision:gateway-connection-pool";
    let discarded_ref = "incident:checkout-latency:attempt:discarded:cdn-cache";
    let policy_ref = "incident:checkout-latency:state:payment-gateway-policy";
    let sibling_ref = "incident:inventory-sync:observation:retry-backoff-saturation";
    let refs = [
        current_ref,
        observation_ref,
        stale_hypothesis_ref,
        decision_ref,
        discarded_ref,
        policy_ref,
        sibling_ref,
    ];

    push(
        &mut items,
        item(
            run_id,
            "conformance.read.wake",
            "read",
            about,
            "holdout-wake-start",
            "Wake the checkout-latency about before the first bounded read.",
            json!({
                "current_ref": null,
                "known_refs": [],
                "last_tool": null,
                "remaining_budget": budget(4),
                "requested_wake": requested_wake(
                    "resume_process",
                    json!({ "mode": "all", "scope": "current_about" }),
                    2200,
                    2
                )
            }),
            read_tools.clone(),
            tool_call(
                "kernel_wake",
                json!({
                    "about": about,
                    "role": "operator",
                    "intent": "resume_process",
                    "dimensions": { "mode": "all", "scope": "current_about" },
                    "depth": 2,
                    "budget": { "tokens": 2200, "depth": 2 }
                }),
            ),
            json!({ "success": true, "observed_refs": [current_ref, observation_ref] }),
            json!({ "bounded": true, "contract_expected": true, "holdout": true }),
        ),
    );

    push(
        &mut items,
        item(
            run_id,
            "conformance.read.wake",
            "read",
            about,
            "holdout-wake-after-empty-near",
            "Wake after a bounded nearby read returned no usable checkout refs.",
            json!({
                "current_ref": current_ref,
                "known_refs": [],
                "last_tool": "kernel_near",
                "last_observed_refs": [],
                "last_result_count": 0,
                "remaining_budget": budget(3),
                "requested_wake": requested_wake(
                    "recover_after_empty_navigation",
                    json!({ "mode": "all", "scope": "current_about" }),
                    2200,
                    2
                )
            }),
            read_tools.clone(),
            tool_call(
                "kernel_wake",
                json!({
                    "about": about,
                    "role": "operator",
                    "intent": "recover_after_empty_navigation",
                    "dimensions": { "mode": "all", "scope": "current_about" },
                    "depth": 2,
                    "budget": { "tokens": 2200, "depth": 2 }
                }),
            ),
            json!({ "success": true, "observed_refs": [current_ref, stale_hypothesis_ref] }),
            json!({ "bounded": true, "contract_expected": true, "holdout": true }),
        ),
    );

    for (step_id, question, answer_policy, dimensions, tokens) in [
        (
            "holdout-ask-current-about",
            "What evidence explains the checkout latency spike?",
            "evidence_or_unknown",
            json!({ "mode": "all", "scope": "current_about" }),
            2300,
        ),
        (
            "holdout-ask-conflicts-only-agents",
            "Which checkout hypotheses conflict with the gateway p99 spike?",
            "show_conflicts",
            json!({ "mode": "only", "scope": "current_about", "include": ["agent:perf", "hypothesis"] }),
            2500,
        ),
        (
            "holdout-ask-about-list",
            "Which related incidents mention retry saturation?",
            "evidence_or_unknown",
            json!({ "mode": "all", "scope": "abouts", "abouts": [about, sibling_about] }),
            3000,
        ),
        (
            "holdout-ask-all-abouts",
            "Did any other incident show a similar saturation pattern?",
            "evidence_or_unknown",
            json!({ "mode": "all", "scope": "all_abouts" }),
            3200,
        ),
    ] {
        push(
            &mut items,
            item(
                run_id,
                "conformance.read.ask",
                "read",
                about,
                step_id,
                question,
                json!({
                    "current_ref": current_ref,
                    "known_refs": refs,
                    "last_tool": "kernel_near",
                    "last_observed_refs": [observation_ref, decision_ref],
                    "remaining_budget": budget(3),
                    "requested_ask": requested_ask(
                        question,
                        answer_policy,
                        dimensions.clone(),
                        tokens,
                        2
                    )
                }),
                read_tools.clone(),
                tool_call(
                    "kernel_ask",
                    json!({
                        "about": about,
                        "answer_policy": answer_policy,
                        "dimensions": dimensions,
                        "question": question,
                        "budget": { "tokens": tokens },
                        "depth": 2
                    }),
                ),
                json!({ "success": true, "observed_refs": [observation_ref, decision_ref, sibling_ref] }),
                json!({ "bounded": true, "contract_expected": true, "holdout": true }),
            ),
        );
    }

    for (step_id, around, dimensions, limit, window, policy) in [
        (
            "holdout-near-ref-shrink-except-discarded",
            json!({ "ref": observation_ref }),
            json!({ "mode": "except", "scope": "current_about", "exclude": ["attempt:discarded"] }),
            json!({ "entries": 5, "tokens": 1400 }),
            json!({ "before_entries": 2, "after_entries": 1 }),
            "shrink_window",
        ),
        (
            "holdout-near-time-expand-about-list",
            json!({ "time": "2026-05-14T09:17:00Z" }),
            json!({ "mode": "all", "scope": "abouts", "abouts": [about, sibling_about] }),
            json!({ "entries": 26, "tokens": 3300 }),
            json!({ "before_entries": 13, "after_entries": 7 }),
            "expand_window",
        ),
        (
            "holdout-near-sequence-shrink-current",
            json!({ "sequence": 18 }),
            json!({ "mode": "all", "scope": "current_about" }),
            json!({ "entries": 4, "tokens": 1100 }),
            json!({ "before_entries": 1, "after_entries": 1 }),
            "shrink_sequence",
        ),
    ] {
        push(
            &mut items,
            item(
                run_id,
                "conformance.read.near",
                "read",
                about,
                step_id,
                "Read nearby checkout memory using the visible requested movement.",
                json!({
                    "current_ref": current_ref,
                    "known_refs": refs,
                    "last_tool": "kernel_ask",
                    "last_observed_refs": [observation_ref],
                    "last_result_count": if policy == "expand_window" { 0 } else { 1 },
                    "last_result_partial": policy == "expand_window",
                    "remaining_budget": budget(3),
                    "requested_move": requested_move("kernel_near", "around", around.clone()),
                    "requested_scope": dimensions.clone(),
                    "requested_bounds": requested_bounds(limit.clone(), window.clone())
                }),
                read_tools.clone(),
                temporal_call_for_about(
                    about,
                    "kernel_near",
                    "around",
                    around,
                    dimensions,
                    limit,
                    window,
                ),
                json!({ "success": true, "observed_refs": [observation_ref, decision_ref] }),
                json!({ "bounded": true, "contract_expected": true, "holdout": true, "policy": policy }),
            ),
        );
    }

    for (tool, cursor_key, cursor, step_id, limit, window) in [
        (
            "kernel_goto",
            "at",
            json!({ "ref": observation_ref }),
            "holdout-goto-ref",
            json!({ "entries": 9, "tokens": 1900 }),
            json!({ "before_entries": 3, "after_entries": 2 }),
        ),
        (
            "kernel_goto",
            "at",
            json!({ "time": "2026-05-14T09:22:00Z" }),
            "holdout-goto-time",
            json!({ "entries": 10, "tokens": 2100 }),
            json!({ "before_entries": 4, "after_entries": 2 }),
        ),
        (
            "kernel_goto",
            "at",
            json!({ "sequence": 21 }),
            "holdout-goto-sequence",
            json!({ "entries": 10, "tokens": 2100 }),
            json!({ "before_entries": 4, "after_entries": 2 }),
        ),
        (
            "kernel_rewind",
            "from",
            json!({ "ref": decision_ref }),
            "holdout-rewind-ref",
            json!({ "entries": 15, "tokens": 2500 }),
            json!({ "before_entries": 9, "after_entries": 0 }),
        ),
        (
            "kernel_rewind",
            "from",
            json!({ "time": "2026-05-14T09:27:00Z" }),
            "holdout-rewind-time",
            json!({ "entries": 15, "tokens": 2500 }),
            json!({ "before_entries": 9, "after_entries": 0 }),
        ),
        (
            "kernel_rewind",
            "from",
            json!({ "sequence": 24 }),
            "holdout-rewind-sequence",
            json!({ "entries": 15, "tokens": 2500 }),
            json!({ "before_entries": 9, "after_entries": 0 }),
        ),
        (
            "kernel_forward",
            "from",
            json!({ "ref": stale_hypothesis_ref }),
            "holdout-forward-ref",
            json!({ "entries": 15, "tokens": 2500 }),
            json!({ "before_entries": 0, "after_entries": 9 }),
        ),
        (
            "kernel_forward",
            "from",
            json!({ "time": "2026-05-14T09:12:00Z" }),
            "holdout-forward-time",
            json!({ "entries": 15, "tokens": 2500 }),
            json!({ "before_entries": 0, "after_entries": 9 }),
        ),
        (
            "kernel_forward",
            "from",
            json!({ "sequence": 17 }),
            "holdout-forward-sequence",
            json!({ "entries": 15, "tokens": 2500 }),
            json!({ "before_entries": 0, "after_entries": 9 }),
        ),
    ] {
        push(
            &mut items,
            item(
                run_id,
                "conformance.read.temporal",
                "read",
                about,
                step_id,
                "Choose the requested temporal movement for checkout replay.",
                json!({
                    "current_ref": decision_ref,
                    "known_refs": refs,
                    "last_tool": "kernel_trace",
                    "last_observed_refs": [observation_ref, decision_ref],
                    "remaining_budget": budget(3),
                    "requested_move": requested_move(tool, cursor_key, cursor.clone()),
                    "requested_scope": json!({ "mode": "all", "scope": "current_about" }),
                    "requested_bounds": requested_bounds(limit.clone(), window.clone())
                }),
                read_tools.clone(),
                temporal_call_for_about(
                    about,
                    tool,
                    cursor_key,
                    cursor,
                    json!({ "mode": "all", "scope": "current_about" }),
                    limit,
                    window,
                ),
                json!({ "success": true, "observed_refs": [observation_ref, decision_ref] }),
                json!({ "bounded": true, "contract_expected": true, "holdout": true }),
            ),
        );
    }

    push(
        &mut items,
        item(
            run_id,
            "conformance.read.trace",
            "read",
            about,
            "holdout-trace-first-page",
            "Trace the checkout path from gateway observation to pool decision.",
            json!({
                "current_ref": observation_ref,
                "trace_target_ref": decision_ref,
                "known_refs": [observation_ref, stale_hypothesis_ref, decision_ref],
                "last_tool": "kernel_forward",
                "last_observed_refs": [decision_ref],
                "last_result_page": null,
                "remaining_budget": budget(2),
                "requested_trace": requested_trace(
                    observation_ref,
                    decision_ref,
                    "Trace why the gateway connection pool decision was selected.",
                    json!({ "entries": 14 })
                )
            }),
            read_tools.clone(),
            tool_call(
                "kernel_trace",
                json!({
                    "from": observation_ref,
                    "to": decision_ref,
                    "goal": "Trace why the gateway connection pool decision was selected.",
                    "role": "operator",
                    "budget": { "depth": 2, "tokens": 2400 },
                    "page": { "entries": 14 }
                }),
            ),
            json!({ "success": true, "observed_refs": [observation_ref, decision_ref], "page": { "entries": 14, "has_more": true, "next_cursor": "14" } }),
            json!({ "bounded": true, "contract_expected": true, "holdout": true }),
        ),
    );

    push(
        &mut items,
        item(
            run_id,
            "conformance.read.trace",
            "read",
            about,
            "holdout-trace-continue-page",
            "Continue the checkout trace using the visible page cursor.",
            json!({
                "current_ref": observation_ref,
                "trace_target_ref": decision_ref,
                "known_refs": [observation_ref, stale_hypothesis_ref, decision_ref],
                "last_tool": "kernel_trace",
                "last_observed_refs": [observation_ref, stale_hypothesis_ref],
                "last_result_page": {
                    "entries": 14,
                    "has_more": true,
                    "next_cursor": "14"
                },
                "last_result_partial": true,
                "remaining_budget": budget(2),
                "requested_trace": requested_trace(
                    observation_ref,
                    decision_ref,
                    "Continue the checkout trace.",
                    json!({ "entries": 14, "cursor": "14" })
                )
            }),
            read_tools.clone(),
            tool_call(
                "kernel_trace",
                json!({
                    "from": observation_ref,
                    "to": decision_ref,
                    "goal": "Continue the checkout trace.",
                    "role": "operator",
                    "budget": { "depth": 2, "tokens": 2400 },
                    "page": { "entries": 14, "cursor": "14" }
                }),
            ),
            json!({ "success": true, "observed_refs": [observation_ref, stale_hypothesis_ref, decision_ref], "page": { "entries": 14, "has_more": false } }),
            json!({ "bounded": true, "contract_expected": true, "holdout": true }),
        ),
    );

    for (step_id, target_ref) in [
        ("holdout-inspect-observation", observation_ref),
        ("holdout-inspect-decision", decision_ref),
    ] {
        push(
            &mut items,
            item(
                run_id,
                "conformance.read.inspect",
                "read",
                about,
                step_id,
                "Inspect a typed checkout node without raw expansion.",
                json!({
                    "current_ref": target_ref,
                    "known_refs": [target_ref, observation_ref, decision_ref],
                    "last_tool": "kernel_trace",
                    "last_observed_refs": [target_ref],
                    "remaining_budget": budget(2),
                    "inspection_request": inspection_request(target_ref)
                }),
                read_tools.clone(),
                tool_call("kernel_inspect", inspection_request(target_ref)),
                json!({ "success": true, "observed_refs": [target_ref] }),
                json!({ "bounded": true, "contract_expected": true, "holdout": true, "raw_access": false }),
            ),
        );
    }

    push(
        &mut items,
        item(
            run_id,
            "conformance.read.failfast",
            "read",
            about,
            "holdout-stop-invalid-about-list-empty",
            "Fail fast when an explicit about-list scope has no about ids.",
            json!({
                "current_ref": current_ref,
                "known_refs": [current_ref],
                "last_tool": "kernel_ask",
                "last_observed_refs": [],
                "proposed_tool": "kernel_ask",
                "proposed_arguments": {
                    "about": about,
                    "dimensions": { "mode": "all", "scope": "abouts", "abouts": [] },
                    "question": "What other incidents are relevant?"
                },
                "remaining_budget": budget(1),
                "requested_stop": requested_stop(
                    "evidence_or_unknown",
                    Vec::new(),
                    "invalid_dimension_selection_abouts_requires_list"
                )
            }),
            read_tools.clone(),
            stop_action(
                "evidence_or_unknown",
                Vec::new(),
                "invalid_dimension_selection_abouts_requires_list",
            ),
            json!({ "success": true, "invalid_tool_call_emitted": false }),
            json!({ "bounded": true, "contract_expected": true, "holdout": true, "expected_failfast": true }),
        ),
    );

    push(
        &mut items,
        item(
            run_id,
            "conformance.read.stop",
            "read",
            about,
            "holdout-stop-sufficient-evidence",
            "Stop when the checkout evidence and decision refs are already sufficient.",
            json!({
                "current_ref": current_ref,
                "known_refs": [current_ref, observation_ref, decision_ref],
                "last_tool": "kernel_inspect",
                "last_observed_refs": [observation_ref, decision_ref],
                "remaining_budget": budget(0),
                "requested_stop": requested_stop(
                    "evidence_or_unknown",
                    vec![observation_ref, decision_ref],
                    "sufficient_evidence"
                )
            }),
            read_tools,
            stop_action(
                "evidence_or_unknown",
                vec![observation_ref, decision_ref],
                "sufficient_evidence",
            ),
            json!({ "success": true, "observed_refs": [observation_ref, decision_ref] }),
            json!({ "bounded": true, "contract_expected": true, "holdout": true, "stop_correct": true }),
        ),
    );

    items
}

fn read_rare_expansion_trajectories(run_id: &str) -> Vec<TrajectoryItem> {
    let mut items = Vec::new();
    let read_tools = kernel_operator_allowed_read_tools();
    let scenarios = [
        ("auth-refresh", "login refresh race", "agent:auth"),
        ("billing-capture", "payment capture retry", "agent:payments"),
        ("search-index", "search index lag", "agent:search"),
        ("deploy-rollback", "deployment rollback", "agent:release"),
        ("cache-invalidation", "cache invalidation", "agent:platform"),
        ("quota-enforcement", "quota enforcement", "agent:runtime"),
        ("webhook-delivery", "webhook delivery", "agent:integrations"),
        ("report-export", "report export", "agent:data"),
    ];

    for (index, (slug, topic, agent_dimension)) in scenarios.iter().enumerate() {
        let about = format!("incident:{slug}");
        let sibling_about = format!("incident:{slug}:sibling");
        let current_ref = format!("{about}:question:root-cause");
        let observation_ref = format!("{about}:observation:primary-signal");
        let stale_ref = format!("{about}:hypothesis:discarded-cause");
        let decision_ref = format!("{about}:decision:final-fix");
        let sibling_ref = format!("{sibling_about}:observation:similar-signal");
        let time = format!("2026-05-16T11:{:02}:00Z", index + 10);
        let later_time = format!("2026-05-16T11:{:02}:30Z", index + 10);
        let sequence = 40 + index;
        let later_sequence = 80 + index;
        let refs = vec![
            current_ref.as_str(),
            observation_ref.as_str(),
            stale_ref.as_str(),
            decision_ref.as_str(),
            sibling_ref.as_str(),
        ];

        let wake_step = format!("rare-wake-start-{slug}");
        push(
            &mut items,
            item(
                run_id,
                "conformance.read.wake",
                "read",
                &about,
                &wake_step,
                &format!("Wake the {topic} memory before choosing a bounded read."),
                json!({
                    "current_ref": null,
                    "known_refs": [],
                    "last_tool": null,
                    "remaining_budget": budget(5),
                    "requested_wake": requested_wake(
                        "resume_process",
                        json!({ "mode": "all", "scope": "current_about" }),
                        2200,
                        2
                    ),
                    "operator_state": {
                        "decision": "wake_current_about",
                        "scenario": slug
                    }
                }),
                read_tools.clone(),
                tool_call(
                    "kernel_wake",
                    json!({
                        "about": about,
                        "role": "operator",
                        "intent": "resume_process",
                        "dimensions": { "mode": "all", "scope": "current_about" },
                        "depth": 2,
                        "budget": { "tokens": 2200, "depth": 2 }
                    }),
                ),
                json!({ "success": true, "observed_refs": [current_ref, observation_ref] }),
                json!({ "bounded": true, "contract_expected": true, "rare_expansion": true }),
            ),
        );

        for (step_suffix, question, answer_policy, dimensions, tokens) in [
            (
                "ask-all-abouts",
                format!("Does another incident show the same {topic} pattern?"),
                "evidence_or_unknown",
                json!({ "mode": "all", "scope": "all_abouts" }),
                3200,
            ),
            (
                "ask-about-list",
                format!("Compare this {topic} incident with the sibling incident."),
                "evidence_or_unknown",
                json!({ "mode": "all", "scope": "abouts", "abouts": [&about, &sibling_about] }),
                3000,
            ),
            (
                "ask-only-agent-decision",
                format!("Which {topic} decision evidence is visible for this agent?"),
                "show_conflicts",
                json!({ "mode": "only", "scope": "current_about", "include": [agent_dimension, "decision"] }),
                2500,
            ),
            (
                "ask-except-discarded",
                format!("What {topic} evidence remains after discarded attempts are excluded?"),
                "evidence_or_unknown",
                json!({ "mode": "except", "scope": "current_about", "exclude": ["attempt:discarded", "scratch"] }),
                2600,
            ),
        ] {
            let step_id = format!("rare-{step_suffix}-{slug}");
            push(
                &mut items,
                item(
                    run_id,
                    "conformance.read.ask",
                    "read",
                    &about,
                    &step_id,
                    &format!("Ask the requested {topic} question using explicit dimensions."),
                    json!({
                        "current_ref": current_ref,
                        "known_refs": refs,
                        "last_tool": "kernel_near",
                        "last_observed_refs": [observation_ref, decision_ref],
                        "remaining_budget": budget(3),
                        "requested_ask": requested_ask(
                            &question,
                            answer_policy,
                            dimensions.clone(),
                            tokens,
                            2
                        ),
                        "operator_state": {
                            "decision": "ask_for_context",
                            "scenario": slug
                        }
                    }),
                    read_tools.clone(),
                    tool_call(
                        "kernel_ask",
                        json!({
                            "about": about,
                            "answer_policy": answer_policy,
                            "dimensions": dimensions,
                            "question": question,
                            "budget": { "tokens": tokens },
                            "depth": 2
                        }),
                    ),
                    json!({ "success": true, "observed_refs": [observation_ref, decision_ref, sibling_ref] }),
                    json!({ "bounded": true, "contract_expected": true, "rare_expansion": true }),
                ),
            );
        }

        for (step_suffix, around, dimensions, limit, window, policy) in [
            (
                "near-time-about-list-expand",
                json!({ "time": time }),
                json!({ "mode": "all", "scope": "abouts", "abouts": [&about, &sibling_about] }),
                json!({ "entries": 24, "tokens": 3200 }),
                json!({ "before_entries": 12, "after_entries": 6 }),
                "expand_window",
            ),
            (
                "near-sequence-only-shrink",
                json!({ "sequence": sequence }),
                json!({ "mode": "only", "scope": "current_about", "include": [agent_dimension, "decision"] }),
                json!({ "entries": 5, "tokens": 1300 }),
                json!({ "before_entries": 2, "after_entries": 1 }),
                "shrink_window",
            ),
            (
                "near-ref-except-shrink",
                json!({ "ref": observation_ref }),
                json!({ "mode": "except", "scope": "current_about", "exclude": ["attempt:discarded"] }),
                json!({ "entries": 6, "tokens": 1500 }),
                json!({ "before_entries": 3, "after_entries": 1 }),
                "shrink_window",
            ),
        ] {
            let step_id = format!("rare-{step_suffix}-{slug}");
            push(
                &mut items,
                item(
                    run_id,
                    "conformance.read.near",
                    "read",
                    &about,
                    &step_id,
                    &format!("Read near {topic} memory with the requested cursor and scope."),
                    json!({
                        "current_ref": current_ref,
                        "known_refs": refs,
                        "last_tool": "kernel_ask",
                        "last_observed_refs": [observation_ref],
                        "last_result_count": if policy == "expand_window" { 0 } else { 1 },
                        "last_result_partial": policy == "expand_window",
                        "remaining_budget": budget(3),
                        "requested_move": requested_move("kernel_near", "around", around.clone()),
                        "requested_scope": dimensions.clone(),
                        "requested_bounds": requested_bounds(limit.clone(), window.clone()),
                        "operator_state": {
                            "decision": policy,
                            "scenario": slug
                        }
                    }),
                    read_tools.clone(),
                    temporal_call_for_about(
                        &about,
                        "kernel_near",
                        "around",
                        around,
                        dimensions,
                        limit,
                        window,
                    ),
                    json!({ "success": true, "observed_refs": [observation_ref, decision_ref] }),
                    json!({ "bounded": true, "contract_expected": true, "rare_expansion": true, "policy": policy }),
                ),
            );
        }

        for (tool, cursor_key, cursor, step_suffix, limit, window) in [
            (
                "kernel_goto",
                "at",
                json!({ "sequence": later_sequence }),
                "goto-sequence",
                json!({ "entries": 10, "tokens": 2200 }),
                json!({ "before_entries": 4, "after_entries": 2 }),
            ),
            (
                "kernel_rewind",
                "from",
                json!({ "time": later_time }),
                "rewind-time",
                json!({ "entries": 14, "tokens": 2400 }),
                json!({ "before_entries": 8, "after_entries": 0 }),
            ),
            (
                "kernel_forward",
                "from",
                json!({ "ref": stale_ref }),
                "forward-ref",
                json!({ "entries": 14, "tokens": 2400 }),
                json!({ "before_entries": 0, "after_entries": 8 }),
            ),
        ] {
            let step_id = format!("rare-{step_suffix}-{slug}");
            push(
                &mut items,
                item(
                    run_id,
                    "conformance.read.temporal",
                    "read",
                    &about,
                    &step_id,
                    &format!("Move through {topic} memory using the requested temporal tool."),
                    json!({
                        "current_ref": decision_ref,
                        "known_refs": refs,
                        "last_tool": "kernel_trace",
                        "last_observed_refs": [observation_ref, decision_ref],
                        "remaining_budget": budget(3),
                        "requested_move": requested_move(tool, cursor_key, cursor.clone()),
                        "requested_scope": json!({ "mode": "all", "scope": "current_about" }),
                        "requested_bounds": requested_bounds(limit.clone(), window.clone()),
                        "operator_state": {
                            "decision": tool,
                            "cursor_key": cursor_key,
                            "scenario": slug
                        }
                    }),
                    read_tools.clone(),
                    temporal_call_for_about(
                        &about,
                        tool,
                        cursor_key,
                        cursor,
                        json!({ "mode": "all", "scope": "current_about" }),
                        limit,
                        window,
                    ),
                    json!({ "success": true, "observed_refs": [observation_ref, decision_ref] }),
                    json!({ "bounded": true, "contract_expected": true, "rare_expansion": true }),
                ),
            );
        }

        let trace_goal = format!("Trace how the {topic} final fix replaced the discarded cause.");
        let trace_first_step = format!("rare-trace-first-{slug}");
        push(
            &mut items,
            item(
                run_id,
                "conformance.read.trace",
                "read",
                &about,
                &trace_first_step,
                &trace_goal,
                json!({
                    "current_ref": observation_ref,
                    "trace_target_ref": decision_ref,
                    "known_refs": [observation_ref, stale_ref, decision_ref],
                    "last_tool": "kernel_forward",
                    "last_observed_refs": [decision_ref],
                    "last_result_page": null,
                    "remaining_budget": budget(2),
                    "requested_trace": requested_trace(
                        &observation_ref,
                        &decision_ref,
                        &trace_goal,
                        json!({ "entries": 14 })
                    ),
                    "operator_state": {
                        "decision": "trace_first_page",
                        "scenario": slug
                    }
                }),
                read_tools.clone(),
                tool_call(
                    "kernel_trace",
                    json!({
                        "from": observation_ref,
                        "to": decision_ref,
                        "goal": trace_goal,
                        "role": "operator",
                        "budget": { "depth": 2, "tokens": 2400 },
                        "page": { "entries": 14 }
                    }),
                ),
                json!({
                    "success": true,
                    "observed_refs": [observation_ref, stale_ref],
                    "page": { "entries": 14, "has_more": true, "next_cursor": "14" }
                }),
                json!({ "bounded": true, "contract_expected": true, "rare_expansion": true, "policy": "trace_first_page" }),
            ),
        );

        let trace_continue_goal = format!("Continue the {topic} trace with the returned cursor.");
        let trace_continue_step = format!("rare-trace-continue-{slug}");
        let trace_cursor = "14";
        push(
            &mut items,
            item(
                run_id,
                "conformance.read.trace",
                "read",
                &about,
                &trace_continue_step,
                &trace_continue_goal,
                json!({
                    "current_ref": observation_ref,
                    "trace_target_ref": decision_ref,
                    "known_refs": [observation_ref, stale_ref, decision_ref],
                    "last_tool": "kernel_trace",
                    "last_observed_refs": [observation_ref, stale_ref],
                    "last_result_page": {
                        "entries": 14,
                        "has_more": true,
                        "next_cursor": trace_cursor
                    },
                    "last_result_partial": true,
                    "remaining_budget": budget(2),
                    "requested_trace": requested_trace(
                        &observation_ref,
                        &decision_ref,
                        &trace_continue_goal,
                        json!({ "entries": 14, "cursor": trace_cursor })
                    ),
                    "operator_state": {
                        "decision": "continue_page",
                        "scenario": slug
                    }
                }),
                read_tools.clone(),
                tool_call(
                    "kernel_trace",
                    json!({
                        "from": observation_ref,
                        "to": decision_ref,
                        "goal": trace_continue_goal,
                        "role": "operator",
                        "budget": { "depth": 2, "tokens": 2400 },
                        "page": { "entries": 14, "cursor": trace_cursor }
                    }),
                ),
                json!({
                    "success": true,
                    "observed_refs": [observation_ref, stale_ref, decision_ref],
                    "page": { "entries": 14, "has_more": false }
                }),
                json!({ "bounded": true, "contract_expected": true, "rare_expansion": true, "policy": "continue_page" }),
            ),
        );
    }

    items
}

fn read_api_mcp_v1_trajectories(run_id: &str) -> Vec<TrajectoryItem> {
    let mut items = read_rare_expansion_trajectories(run_id);

    let topics = fixture_or_panic(
        "read_api_mcp_topics.json",
        read_api_mcp_topics_from_json(include_str!(
            "../../fixtures/conformance/read_api_mcp_topics.json"
        )),
    );
    let variants = fixture_or_panic(
        "read_api_mcp_variants.json",
        read_api_mcp_variants_from_json(include_str!(
            "../../fixtures/conformance/read_api_mcp_variants.json"
        )),
    );

    let mut index = 0usize;
    for topic in &topics {
        for variant in &variants {
            index += 1;
            let slug = format!("{}-{}-{index:02}", topic.slug(), variant.slug());
            let agent_dimension = fixture_or_panic(
                "read_api_mcp.agent_dimension",
                topic.agent_dimension_for(variant),
            );
            push_read_api_mcp_v1_scenario(
                &mut items,
                run_id,
                &slug,
                topic.title(),
                &agent_dimension,
                variant.intent(),
                index,
            );
        }
    }

    items
}

#[allow(clippy::too_many_arguments)]
fn push_read_api_mcp_v1_scenario(
    items: &mut Vec<TrajectoryItem>,
    run_id: &str,
    slug: &str,
    topic: &ReadTopicTitle,
    agent_dimension: &ReadAgentDimension,
    intent: &ReadIntent,
    index: usize,
) {
    let read_tools = kernel_operator_allowed_read_tools();
    let about = format!("incident:read-api:{slug}");
    let sibling_about = format!("incident:read-api:{slug}:sibling");
    let current_ref = format!("{about}:question:root-cause");
    let observation_ref = format!("{about}:observation:primary-signal");
    let stale_ref = format!("{about}:hypothesis:discarded-cause");
    let decision_ref = format!("{about}:decision:final-fix");
    let supporting_ref = format!("{about}:evidence:supporting-detail");
    let sibling_ref = format!("{sibling_about}:observation:similar-signal");
    let minute = 10 + (index % 40);
    let time = format!("2026-05-16T12:{minute:02}:00Z");
    let later_time = format!("2026-05-16T12:{minute:02}:30Z");
    let sequence = 100 + index;
    let later_sequence = 200 + index;
    let refs = vec![
        current_ref.as_str(),
        observation_ref.as_str(),
        stale_ref.as_str(),
        decision_ref.as_str(),
        supporting_ref.as_str(),
        sibling_ref.as_str(),
    ];

    let wake_step = format!("read-api-{slug}-wake");
    push(
        items,
        item(
            run_id,
            "conformance.read.wake",
            "read",
            &about,
            &wake_step,
            &format!("Wake {topic} memory before choosing a bounded read action for {intent}."),
            json!({
                "current_ref": null,
                "known_refs": [],
                "last_tool": null,
                "remaining_budget": budget(5),
                "requested_wake": requested_wake(
                    "resume_process",
                    json!({ "mode": "all", "scope": "current_about" }),
                    2200 + (index % 3) * 200,
                    2
                ),
                "operator_state": {
                    "decision": "wake_current_about",
                    "scenario": slug,
                    "intent": intent.as_str()
                }
            }),
            read_tools.clone(),
            tool_call(
                "kernel_wake",
                json!({
                    "about": about,
                    "role": "operator",
                    "intent": "resume_process",
                    "dimensions": { "mode": "all", "scope": "current_about" },
                    "depth": 2,
                    "budget": { "tokens": 2200 + (index % 3) * 200, "depth": 2 }
                }),
            ),
            json!({ "success": true, "observed_refs": [current_ref, observation_ref] }),
            json!({ "bounded": true, "contract_expected": true }),
        ),
    );
    tag_read_api_mcp_v1(items, "wake", "current_about", slug);

    let ask_cases = [
        (
            "ask-current-about",
            format!("What evidence explains the {topic} incident?"),
            "evidence_or_unknown",
            json!({ "mode": "all", "scope": "current_about" }),
            2400,
        ),
        (
            "ask-explicit-abouts",
            format!("Does the sibling incident show the same {topic} pattern?"),
            "evidence_or_unknown",
            json!({ "mode": "all", "scope": "abouts", "abouts": [&about, &sibling_about] }),
            3000,
        ),
        (
            "ask-all-abouts",
            format!("Search all abouts intentionally for related {topic} evidence."),
            "evidence_or_unknown",
            json!({ "mode": "all", "scope": "all_abouts" }),
            3200,
        ),
        (
            "ask-conflicts-only",
            format!("Which {topic} hypotheses conflict with the primary signal?"),
            "show_conflicts",
            json!({ "mode": "only", "scope": "current_about", "include": [agent_dimension.as_str(), "hypothesis", "decision"] }),
            2600,
        ),
    ];
    for (suffix, question, answer_policy, dimensions, tokens) in ask_cases {
        let step_id = format!("read-api-{slug}-{suffix}");
        push(
            items,
            item(
                run_id,
                "conformance.read.ask",
                "read",
                &about,
                &step_id,
                &format!("Ask a bounded {topic} question with explicit scope for {intent}."),
                json!({
                    "current_ref": current_ref,
                    "known_refs": refs,
                    "last_tool": "kernel_wake",
                    "last_observed_refs": [current_ref, observation_ref],
                    "remaining_budget": budget(4),
                    "requested_ask": requested_ask(
                        &question,
                        answer_policy,
                        dimensions.clone(),
                        tokens,
                        2
                    ),
                    "operator_state": {
                        "decision": "ask_for_context",
                        "scenario": slug,
                        "intent": intent.as_str()
                    }
                }),
                read_tools.clone(),
                tool_call(
                    "kernel_ask",
                    json!({
                        "about": about,
                        "answer_policy": answer_policy,
                        "dimensions": dimensions,
                        "question": question,
                        "budget": { "tokens": tokens },
                        "depth": 2
                    }),
                ),
                json!({ "success": true, "observed_refs": [observation_ref, decision_ref, sibling_ref] }),
                json!({ "bounded": true, "contract_expected": true }),
            ),
        );
        tag_read_api_mcp_v1(items, "ask", suffix, slug);
    }

    let near_cases = [
        (
            "near-ref-shrink",
            json!({ "ref": observation_ref }),
            json!({ "mode": "except", "scope": "current_about", "exclude": ["attempt:discarded"] }),
            json!({ "entries": 5 + (index % 3), "tokens": 1300 + (index % 4) * 100 }),
            json!({ "before_entries": 2 + (index % 3), "after_entries": 1 }),
            "shrink_window",
        ),
        (
            "near-time-expand",
            json!({ "time": time }),
            json!({ "mode": "all", "scope": "abouts", "abouts": [&about, &sibling_about] }),
            json!({ "entries": 20 + (index % 6), "tokens": 3000 + (index % 4) * 200 }),
            json!({ "before_entries": 9 + (index % 5), "after_entries": 4 + (index % 3) }),
            "expand_window",
        ),
        (
            "near-sequence-current",
            json!({ "sequence": sequence }),
            json!({ "mode": "all", "scope": "current_about" }),
            json!({ "entries": 12, "tokens": 2200 }),
            json!({ "before_entries": 6, "after_entries": 2 }),
            "bounded_window",
        ),
    ];
    for (suffix, around, dimensions, limit, window, policy) in near_cases {
        let step_id = format!("read-api-{slug}-{suffix}");
        push(
            items,
            item(
                run_id,
                "conformance.read.near",
                "read",
                &about,
                &step_id,
                &format!("Read near {topic} using the requested cursor and scope for {intent}."),
                json!({
                    "current_ref": current_ref,
                    "known_refs": refs,
                    "last_tool": "kernel_ask",
                    "last_observed_refs": [observation_ref],
                    "last_result_count": if policy == "expand_window" { 0 } else { 2 },
                    "last_result_partial": policy == "expand_window",
                    "remaining_budget": budget(3),
                    "requested_move": requested_move("kernel_near", "around", around.clone()),
                    "requested_scope": dimensions.clone(),
                    "requested_bounds": requested_bounds(limit.clone(), window.clone()),
                    "operator_state": {
                        "decision": policy,
                        "scenario": slug,
                        "intent": intent.as_str()
                    }
                }),
                read_tools.clone(),
                temporal_call_for_about(
                    &about,
                    "kernel_near",
                    "around",
                    around,
                    dimensions,
                    limit,
                    window,
                ),
                json!({ "success": true, "observed_refs": [observation_ref, supporting_ref] }),
                json!({ "bounded": true, "contract_expected": true, "policy": policy }),
            ),
        );
        tag_read_api_mcp_v1(items, "near", suffix, slug);
    }

    let goto_cursor = match index % 3 {
        0 => json!({ "ref": observation_ref }),
        1 => json!({ "time": time }),
        _ => json!({ "sequence": sequence }),
    };
    let rewind_cursor = match index % 3 {
        0 => json!({ "time": later_time }),
        1 => json!({ "sequence": later_sequence }),
        _ => json!({ "ref": decision_ref }),
    };
    let forward_cursor = match index % 3 {
        0 => json!({ "sequence": sequence }),
        1 => json!({ "ref": stale_ref }),
        _ => json!({ "time": time }),
    };
    let temporal_cases = [
        (
            "kernel_goto",
            "at",
            goto_cursor,
            "goto",
            json!({ "entries": 8 + (index % 4), "tokens": 1800 + (index % 3) * 100 }),
            json!({ "before_entries": 3, "after_entries": 2 }),
        ),
        (
            "kernel_rewind",
            "from",
            rewind_cursor,
            "rewind",
            json!({ "entries": 14 + (index % 3), "tokens": 2400 }),
            json!({ "before_entries": 8 + (index % 3), "after_entries": 0 }),
        ),
        (
            "kernel_forward",
            "from",
            forward_cursor,
            "forward",
            json!({ "entries": 14 + (index % 3), "tokens": 2400 }),
            json!({ "before_entries": 0, "after_entries": 8 + (index % 3) }),
        ),
    ];
    for (tool, cursor_key, cursor, suffix, limit, window) in temporal_cases {
        let step_id = format!("read-api-{slug}-{suffix}");
        push(
            items,
            item(
                run_id,
                "conformance.read.temporal",
                "read",
                &about,
                &step_id,
                &format!(
                    "Move through {topic} memory with the requested temporal action for {intent}."
                ),
                json!({
                    "current_ref": decision_ref,
                    "known_refs": refs,
                    "last_tool": "kernel_near",
                    "last_observed_refs": [observation_ref, decision_ref],
                    "remaining_budget": budget(3),
                    "requested_move": requested_move(tool, cursor_key, cursor.clone()),
                    "requested_scope": json!({ "mode": "all", "scope": "current_about" }),
                    "requested_bounds": requested_bounds(limit.clone(), window.clone()),
                    "operator_state": {
                        "decision": tool,
                        "scenario": slug,
                        "intent": intent.as_str()
                    }
                }),
                read_tools.clone(),
                temporal_call_for_about(
                    &about,
                    tool,
                    cursor_key,
                    cursor,
                    json!({ "mode": "all", "scope": "current_about" }),
                    limit,
                    window,
                ),
                json!({ "success": true, "observed_refs": [observation_ref, decision_ref] }),
                json!({ "bounded": true, "contract_expected": true }),
            ),
        );
        tag_read_api_mcp_v1(items, "temporal", suffix, slug);
    }

    let trace_goal =
        format!("Trace why the {topic} final decision replaced the discarded cause for {intent}.");
    let trace_cursor = (12 + (index % 5)).to_string();
    let trace_first_step = format!("read-api-{slug}-trace-first");
    push(
        items,
        item(
            run_id,
            "conformance.read.trace",
            "read",
            &about,
            &trace_first_step,
            &trace_goal,
            json!({
                "current_ref": observation_ref,
                "trace_target_ref": decision_ref,
                "known_refs": [observation_ref, stale_ref, decision_ref],
                "last_tool": "kernel_forward",
                "last_observed_refs": [decision_ref],
                "last_result_page": null,
                "remaining_budget": budget(2),
                "requested_trace": requested_trace(
                    &observation_ref,
                    &decision_ref,
                    &trace_goal,
                    json!({ "entries": 12 + (index % 5) })
                ),
                "operator_state": {
                    "decision": "trace_first_page",
                    "scenario": slug,
                    "intent": intent.as_str()
                }
            }),
            read_tools.clone(),
            tool_call(
                "kernel_trace",
                json!({
                    "from": observation_ref,
                    "to": decision_ref,
                    "goal": trace_goal,
                    "role": "operator",
                    "budget": { "depth": 2, "tokens": 2400 },
                    "page": { "entries": 12 + (index % 5) }
                }),
            ),
            json!({
                "success": true,
                "observed_refs": [observation_ref, stale_ref],
                "page": { "entries": 12 + (index % 5), "has_more": true, "next_cursor": trace_cursor }
            }),
            json!({ "bounded": true, "contract_expected": true, "policy": "trace_first_page" }),
        ),
    );
    tag_read_api_mcp_v1(items, "trace", "first_page", slug);

    let trace_continue_step = format!("read-api-{slug}-trace-continue");
    let trace_continue_goal =
        format!("Continue the {topic} trace with the returned cursor for {intent}.");
    push(
        items,
        item(
            run_id,
            "conformance.read.trace",
            "read",
            &about,
            &trace_continue_step,
            &trace_continue_goal,
            json!({
                "current_ref": observation_ref,
                "trace_target_ref": decision_ref,
                "known_refs": [observation_ref, stale_ref, decision_ref],
                "last_tool": "kernel_trace",
                "last_observed_refs": [observation_ref, stale_ref],
                "last_result_page": {
                    "entries": 12 + (index % 5),
                    "has_more": true,
                    "next_cursor": trace_cursor
                },
                "last_result_partial": true,
                "remaining_budget": budget(2),
                "requested_trace": requested_trace(
                    &observation_ref,
                    &decision_ref,
                    &trace_continue_goal,
                    json!({ "entries": 12 + (index % 5), "cursor": trace_cursor })
                ),
                "operator_state": {
                    "decision": "continue_page",
                    "scenario": slug,
                    "intent": intent.as_str()
                }
            }),
            read_tools.clone(),
            tool_call(
                "kernel_trace",
                json!({
                    "from": observation_ref,
                    "to": decision_ref,
                    "goal": trace_continue_goal,
                    "role": "operator",
                    "budget": { "depth": 2, "tokens": 2400 },
                    "page": { "entries": 12 + (index % 5), "cursor": trace_cursor }
                }),
            ),
            json!({
                "success": true,
                "observed_refs": [observation_ref, stale_ref, decision_ref],
                "page": { "entries": 12 + (index % 5), "has_more": false }
            }),
            json!({ "bounded": true, "contract_expected": true, "policy": "trace_continue" }),
        ),
    );
    tag_read_api_mcp_v1(items, "trace", "continue_page", slug);

    for (suffix, target_ref) in [
        ("inspect-observation", observation_ref.as_str()),
        ("inspect-decision", decision_ref.as_str()),
    ] {
        let step_id = format!("read-api-{slug}-{suffix}");
        push(
            items,
            item(
                run_id,
                "conformance.read.inspect",
                "read",
                &about,
                &step_id,
                &format!("Inspect a {topic} node without raw expansion for {intent}."),
                json!({
                    "current_ref": target_ref,
                    "known_refs": [target_ref, observation_ref, decision_ref],
                    "last_tool": "kernel_trace",
                    "last_observed_refs": [target_ref],
                    "remaining_budget": budget(2),
                    "inspection_request": inspection_request(target_ref),
                    "operator_state": {
                        "decision": "inspect_detail",
                        "scenario": slug,
                        "intent": intent.as_str()
                    }
                }),
                read_tools.clone(),
                tool_call("kernel_inspect", inspection_request(target_ref)),
                json!({ "success": true, "observed_refs": [target_ref] }),
                json!({ "bounded": true, "contract_expected": true, "raw_access": false }),
            ),
        );
        tag_read_api_mcp_v1(items, "inspect", suffix, slug);
    }

    let stop_step = format!("read-api-{slug}-stop-sufficient");
    let stop_reason = format!("sufficient_evidence_for_{slug}");
    push(
        items,
        item(
            run_id,
            "conformance.read.stop",
            "read",
            &about,
            &stop_step,
            &format!(
                "Stop because the {topic} evidence and final decision are already visible for {intent}."
            ),
            json!({
                "current_ref": current_ref,
                "known_refs": [current_ref, observation_ref, decision_ref],
                "last_tool": "kernel_inspect",
                "last_observed_refs": [observation_ref, decision_ref],
                "remaining_budget": budget(0),
                "requested_stop": requested_stop(
                    "evidence_or_unknown",
                    vec![observation_ref.as_str(), decision_ref.as_str()],
                    &stop_reason
                ),
                "operator_state": {
                    "decision": "stop_sufficient",
                    "scenario": slug,
                        "intent": intent.as_str()
                }
            }),
            read_tools.clone(),
            stop_action(
                "evidence_or_unknown",
                vec![observation_ref.as_str(), decision_ref.as_str()],
                &stop_reason,
            ),
            json!({ "success": true, "observed_refs": [observation_ref, decision_ref] }),
            json!({ "bounded": true, "contract_expected": true, "stop_correct": true }),
        ),
    );
    tag_read_api_mcp_v1(items, "stop", "sufficient_evidence", slug);

    let failfast_step = format!("read-api-{slug}-stop-invalid-about-list");
    push(
        items,
        item(
            run_id,
            "conformance.read.failfast",
            "read",
            &about,
            &failfast_step,
            &format!("Fail fast when explicit about-list scope is empty for {topic} / {intent}."),
            json!({
                "current_ref": current_ref,
                "known_refs": [current_ref],
                "last_tool": "kernel_ask",
                "last_observed_refs": [],
                "proposed_tool": "kernel_ask",
                "proposed_arguments": {
                    "about": about,
                    "dimensions": { "mode": "all", "scope": "abouts", "abouts": [] },
                    "question": "What related memory should be searched?"
                },
                "remaining_budget": budget(1),
                "requested_stop": requested_stop(
                    "evidence_or_unknown",
                    Vec::new(),
                    "invalid_dimension_selection_abouts_requires_list"
                ),
                "operator_state": {
                    "decision": "stop_invalid_arguments",
                    "scenario": slug,
                    "intent": intent.as_str()
                }
            }),
            read_tools,
            stop_action(
                "evidence_or_unknown",
                Vec::new(),
                "invalid_dimension_selection_abouts_requires_list",
            ),
            json!({ "success": true, "invalid_tool_call_emitted": false }),
            json!({ "bounded": true, "contract_expected": true, "expected_failfast": true }),
        ),
    );
    tag_read_api_mcp_v1(items, "stop", "failfast_invalid_scope", slug);
}

fn tag_read_api_mcp_v1(
    items: &mut [TrajectoryItem],
    synthetic_family: &str,
    variant: &str,
    scenario: &str,
) {
    let Some(item) = items.last_mut() else {
        return;
    };
    if let Some(quality) = item.quality.as_object_mut() {
        quality.insert(
            "label_source".to_string(),
            json!("deterministic_synthetic_policy"),
        );
        quality.insert("synthetic_suite".to_string(), json!("read-api-mcp-v1"));
        quality.insert("synthetic_family".to_string(), json!(synthetic_family));
        quality.insert("synthetic_variant".to_string(), json!(variant));
        quality.insert("synthetic_scenario".to_string(), json!(scenario));
    }
}

fn writer_pre_read_trajectories(run_id: &str) -> Vec<TrajectoryItem> {
    let mut items = Vec::new();
    let read_tools = kernel_operator_allowed_writer_pre_read_tools();
    let about = "incident:writer-pre-read";
    let entry_question = "incident:writer-pre-read:subtask:4:question";
    let entry_answer = "incident:writer-pre-read:subtask:4:answer";
    let same_question = "incident:writer-pre-read:subtask:4:question";
    let previous_answer = "incident:writer-pre-read:subtask:3:answer";
    let previous_question = "incident:writer-pre-read:subtask:3:question";
    let older_answer = "incident:writer-pre-read:subtask:2:answer";

    let question_candidates = writer_pre_read_candidate_details(vec![
        (
            previous_answer,
            "previous_subtask_answer",
            10,
            "question_follows_previous_answer",
        ),
        (
            previous_question,
            "previous_subtask_question",
            30,
            "question_refines_previous_question",
        ),
        (
            older_answer,
            "older_subtask_answer",
            50,
            "question_uses_older_answer",
        ),
    ]);
    let answer_candidates = writer_pre_read_candidate_details(vec![
        (
            same_question,
            "same_subtask_question",
            10,
            "answer_addresses_question",
        ),
        (
            previous_answer,
            "previous_subtask_answer",
            30,
            "answer_uses_prior_answer",
        ),
        (
            previous_question,
            "previous_subtask_question",
            40,
            "answer_uses_prior_question",
        ),
    ]);

    push_writer_pre_read_near(
        &mut items,
        run_id,
        &read_tools,
        about,
        "writer-question-near-previous-answer-shrink",
        entry_question,
        previous_answer,
        question_candidates.clone(),
        json!({ "entries": 6, "tokens": 1200 }),
        json!({ "before_entries": 3, "after_entries": 0 }),
        "Use a tight near read around the previous answer before writing why this question follows it.",
        vec![previous_answer, previous_question],
    );
    push_writer_pre_read_inspect(
        &mut items,
        run_id,
        &read_tools,
        about,
        "writer-question-inspect-previous-answer-after-near",
        entry_question,
        previous_answer,
        question_candidates.clone(),
        "Inspect the prior answer after near surfaced it as the likely relation target.",
    );
    push_writer_pre_read_trace(
        &mut items,
        run_id,
        &read_tools,
        about,
        "writer-question-trace-to-previous-answer-after-inspect",
        entry_question,
        previous_answer,
        question_candidates.clone(),
        "Trace from the new question to the prior answer to prove the relation path.",
    );
    push_writer_pre_read_near(
        &mut items,
        run_id,
        &read_tools,
        about,
        "writer-question-near-older-answer-expand",
        entry_question,
        older_answer,
        question_candidates,
        json!({ "entries": 24, "tokens": 3200 }),
        json!({ "before_entries": 12, "after_entries": 0 }),
        "Use a wider near read when the relation candidate is older than the immediate prior turn.",
        vec![older_answer, previous_answer, previous_question],
    );

    push_writer_pre_read_near(
        &mut items,
        run_id,
        &read_tools,
        about,
        "writer-answer-near-same-question-shrink",
        entry_answer,
        same_question,
        answer_candidates.clone(),
        json!({ "entries": 6, "tokens": 1200 }),
        json!({ "before_entries": 3, "after_entries": 0 }),
        "Use a tight near read around the same subtask question before writing an answer relation.",
        vec![same_question, previous_answer],
    );
    push_writer_pre_read_inspect(
        &mut items,
        run_id,
        &read_tools,
        about,
        "writer-answer-inspect-same-question-after-near",
        entry_answer,
        same_question,
        answer_candidates.clone(),
        "Inspect the same subtask question after near surfaced it as the relation target.",
    );
    push_writer_pre_read_trace(
        &mut items,
        run_id,
        &read_tools,
        about,
        "writer-answer-trace-to-same-question-after-inspect",
        entry_answer,
        same_question,
        answer_candidates.clone(),
        "Trace from the new answer to the same question to prove answer-addresses-question.",
    );
    push_writer_pre_read_near(
        &mut items,
        run_id,
        &read_tools,
        about,
        "writer-answer-near-previous-answer-expand",
        entry_answer,
        previous_answer,
        answer_candidates,
        json!({ "entries": 24, "tokens": 3200 }),
        json!({ "before_entries": 12, "after_entries": 0 }),
        "Use a wider near read when an answer may reuse a previous answer as relation evidence.",
        vec![previous_answer, previous_question, same_question],
    );

    items
}

fn writer_pre_read_v2_trajectories(run_id: &str) -> Vec<TrajectoryItem> {
    let mut items = writer_pre_read_trajectories(run_id);
    let read_tools = kernel_operator_allowed_writer_pre_read_tools();
    let about = "incident:writer-pre-read";
    let entry_question = "incident:writer-pre-read:subtask:5:question";
    let entry_answer = "incident:writer-pre-read:subtask:5:answer";
    let same_question = "incident:writer-pre-read:subtask:5:question";
    let previous_answer = "incident:writer-pre-read:subtask:4:answer";
    let previous_question = "incident:writer-pre-read:subtask:4:question";
    let older_answer = "incident:writer-pre-read:subtask:3:answer";
    let older_question = "incident:writer-pre-read:subtask:3:question";
    let question_trace_cursor = "16";
    let answer_trace_cursor = "16";

    let ambiguous_question_candidates = writer_pre_read_candidate_details(vec![
        (
            previous_answer,
            "previous_subtask_answer",
            10,
            "question_may_depend_on_previous_answer",
        ),
        (
            older_answer,
            "older_subtask_answer",
            10,
            "question_may_reopen_older_answer",
        ),
        (
            previous_question,
            "previous_subtask_question",
            20,
            "question_may_refine_previous_question",
        ),
    ]);
    let ambiguous_answer_candidates = writer_pre_read_candidate_details(vec![
        (
            same_question,
            "same_subtask_question",
            10,
            "answer_may_address_same_question",
        ),
        (
            previous_answer,
            "previous_subtask_answer",
            10,
            "answer_may_supersede_previous_answer",
        ),
        (
            older_question,
            "older_subtask_question",
            30,
            "answer_may_reuse_older_question_context",
        ),
    ]);

    push_writer_pre_read_inspect_ambiguous(
        &mut items,
        run_id,
        &read_tools,
        about,
        "writer-question-inspect-ambiguous-previous-answer-after-near",
        entry_question,
        previous_answer,
        ambiguous_question_candidates.clone(),
        vec![previous_answer, older_answer, previous_question],
        "Inspect the highest-ranked candidate when near returns multiple plausible relation targets.",
    );
    push_writer_pre_read_inspect_ambiguous(
        &mut items,
        run_id,
        &read_tools,
        about,
        "writer-answer-inspect-ambiguous-same-question-after-near",
        entry_answer,
        same_question,
        ambiguous_answer_candidates.clone(),
        vec![same_question, previous_answer, older_question],
        "Inspect the same question before choosing whether the answer addresses or supersedes prior memory.",
    );

    push_writer_pre_read_trace_continue(
        &mut items,
        run_id,
        &read_tools,
        about,
        "writer-question-trace-continue-after-partial-page",
        entry_question,
        previous_answer,
        ambiguous_question_candidates.clone(),
        question_trace_cursor,
        vec![entry_question, previous_answer, previous_question],
        "Continue the trace page instead of deciding from an incomplete relation path.",
    );
    push_writer_pre_read_trace_continue(
        &mut items,
        run_id,
        &read_tools,
        about,
        "writer-answer-trace-continue-after-partial-page",
        entry_answer,
        same_question,
        ambiguous_answer_candidates.clone(),
        answer_trace_cursor,
        vec![entry_answer, same_question, previous_answer],
        "Continue the trace page when the writer still needs proof before writing the relation.",
    );

    push_writer_pre_read_stop(
        &mut items,
        run_id,
        &read_tools,
        about,
        "writer-question-stop-sufficient-after-trace",
        entry_question,
        previous_answer,
        ambiguous_question_candidates,
        vec![entry_question, previous_answer, previous_question],
        vec![previous_answer, previous_question],
        "Stop reading because the candidate target and supporting prior question are visible enough to write the next relation.",
    );
    push_writer_pre_read_stop(
        &mut items,
        run_id,
        &read_tools,
        about,
        "writer-answer-stop-sufficient-after-trace",
        entry_answer,
        same_question,
        ambiguous_answer_candidates,
        vec![entry_answer, same_question, previous_answer],
        vec![same_question, previous_answer],
        "Stop reading because the writer has enough evidence to connect the answer without inventing a relation.",
    );

    items
}

fn writer_pre_read_v3_trajectories(run_id: &str) -> Vec<TrajectoryItem> {
    let mut items = Vec::new();
    let read_tools = kernel_operator_allowed_writer_pre_read_tools();

    let topics = fixture_or_panic(
        "writer_pre_read_topics.json",
        writer_pre_read_topics_from_json(include_str!(
            "../../fixtures/conformance/writer_pre_read_topics.json"
        )),
    );
    let variants = fixture_or_panic(
        "writer_pre_read_variants.json",
        writer_pre_read_variants_from_json(include_str!(
            "../../fixtures/conformance/writer_pre_read_variants.json"
        )),
    );

    let mut case_index = 0usize;
    for topic in &topics {
        for variant in &variants {
            case_index += 1;
            let topic_slug = topic.slug();
            let topic_title = topic.title();
            let question_hint = topic.question_hint();
            let answer_hint = topic.answer_hint();
            let variant_slug = variant.slug();
            let question_primary_role = variant.question_primary_role();
            let answer_secondary_role = variant.answer_secondary_role();
            let variant_reason = variant.reason();
            let slug = format!("{topic_slug}-{variant_slug}-{case_index:02}");
            let about = format!("incident:{run_id}:writer-pre-read-v3:{slug}");
            let entry_question = format!("{about}:subtask:5:question");
            let entry_answer = format!("{about}:subtask:5:answer");
            let same_question = entry_question.clone();
            let previous_answer = format!("{about}:subtask:4:answer");
            let previous_question = format!("{about}:subtask:4:question");
            let older_answer = format!("{about}:subtask:3:answer");
            let older_question = format!("{about}:subtask:3:question");
            let trace_cursor_question = "16";
            let trace_cursor_answer = "16";
            let priority_offset = (case_index % 5) as u64;

            let question_primary_ref = if question_primary_role.selects_older_subtask_answer() {
                older_answer.as_str()
            } else {
                previous_answer.as_str()
            };
            let answer_primary_ref = same_question.as_str();
            let answer_secondary_ref = if answer_secondary_role.selects_previous_subtask_answer() {
                previous_answer.as_str()
            } else {
                older_question.as_str()
            };

            let question_candidates = writer_pre_read_candidate_details(vec![
                (
                    question_primary_ref,
                    question_primary_role.as_str(),
                    10 + priority_offset,
                    question_hint.as_str(),
                ),
                (
                    previous_question.as_str(),
                    "previous_subtask_question",
                    24 + priority_offset,
                    "question_may_refine_previous_question",
                ),
                (
                    older_answer.as_str(),
                    "older_subtask_answer",
                    35 + priority_offset,
                    "question_may_reopen_older_answer",
                ),
                (
                    older_question.as_str(),
                    "older_subtask_question",
                    50 + priority_offset,
                    "question_background_context_only",
                ),
            ]);
            let answer_candidates = writer_pre_read_candidate_details(vec![
                (
                    answer_primary_ref,
                    "same_subtask_question",
                    10 + priority_offset,
                    answer_hint.as_str(),
                ),
                (
                    answer_secondary_ref,
                    answer_secondary_role.as_str(),
                    18 + priority_offset,
                    "answer_may_reuse_prior_context",
                ),
                (
                    previous_question.as_str(),
                    "previous_subtask_question",
                    31 + priority_offset,
                    "answer_may_compare_prior_question",
                ),
                (
                    older_answer.as_str(),
                    "older_subtask_answer",
                    44 + priority_offset,
                    "answer_background_context_only",
                ),
            ]);

            let tight_entries = 6 + (case_index % 3) as u64;
            let tight_before = 2 + (case_index % 3) as u64;
            let wide_entries = 18 + (case_index % 7) as u64;
            let wide_before = 8 + (case_index % 5) as u64;

            push_writer_pre_read_near(
                &mut items,
                run_id,
                &read_tools,
                &about,
                &format!("writer-v3-{slug}-question-near-context"),
                &entry_question,
                question_primary_ref,
                question_candidates.clone(),
                json!({ "entries": wide_entries, "tokens": 2600 + (case_index % 4) * 200 }),
                json!({ "before_entries": wide_before, "after_entries": 0 }),
                &format!(
                    "Read bounded nearby context for the {topic_title} question before choosing a memory relation; {variant_reason}."
                ),
                vec![
                    question_primary_ref,
                    previous_question.as_str(),
                    older_answer.as_str(),
                ],
            );
            tag_writer_pre_read_v3(
                &mut items,
                "near_expand_before_relation",
                variant_slug.as_str(),
                topic_slug.as_str(),
            );

            push_writer_pre_read_inspect_ambiguous(
                &mut items,
                run_id,
                &read_tools,
                &about,
                &format!("writer-v3-{slug}-question-inspect-candidate"),
                &entry_question,
                question_primary_ref,
                question_candidates.clone(),
                vec![
                    question_primary_ref,
                    previous_question.as_str(),
                    older_answer.as_str(),
                ],
                &format!(
                    "Inspect the strongest {topic_title} question candidate after near returned several plausible relation targets."
                ),
            );
            tag_writer_pre_read_v3(
                &mut items,
                "inspect_after_near_ambiguous",
                variant_slug.as_str(),
                topic_slug.as_str(),
            );

            push_writer_pre_read_trace(
                &mut items,
                run_id,
                &read_tools,
                &about,
                &format!("writer-v3-{slug}-question-trace-first"),
                &entry_question,
                question_primary_ref,
                question_candidates.clone(),
                &format!(
                    "Trace the {topic_title} question relation after inspection because the writer still needs path proof."
                ),
            );
            tag_writer_pre_read_v3(
                &mut items,
                "trace_first_after_inspect",
                variant_slug.as_str(),
                topic_slug.as_str(),
            );

            push_writer_pre_read_trace_continue(
                &mut items,
                run_id,
                &read_tools,
                &about,
                &format!("writer-v3-{slug}-question-trace-continue"),
                &entry_question,
                question_primary_ref,
                question_candidates.clone(),
                trace_cursor_question,
                vec![
                    entry_question.as_str(),
                    question_primary_ref,
                    previous_question.as_str(),
                ],
                &format!(
                    "Continue the {topic_title} question trace because the previous page was partial."
                ),
            );
            tag_writer_pre_read_v3(
                &mut items,
                "trace_continue_after_partial_page",
                variant_slug.as_str(),
                topic_slug.as_str(),
            );

            push_writer_pre_read_stop(
                &mut items,
                run_id,
                &read_tools,
                &about,
                &format!("writer-v3-{slug}-question-stop-sufficient"),
                &entry_question,
                question_primary_ref,
                question_candidates.clone(),
                vec![
                    entry_question.as_str(),
                    question_primary_ref,
                    previous_question.as_str(),
                ],
                vec![question_primary_ref, previous_question.as_str()],
                &format!("sufficient_evidence_for_{topic_slug}_{variant_slug}_question_relation"),
            );
            tag_writer_pre_read_v3(
                &mut items,
                "stop_after_complete_trace",
                variant_slug.as_str(),
                topic_slug.as_str(),
            );

            push_writer_pre_read_near(
                &mut items,
                run_id,
                &read_tools,
                &about,
                &format!("writer-v3-{slug}-answer-near-question"),
                &entry_answer,
                answer_primary_ref,
                answer_candidates.clone(),
                json!({ "entries": tight_entries, "tokens": 1200 + (case_index % 4) * 100 }),
                json!({ "before_entries": tight_before, "after_entries": 0 }),
                &format!(
                    "Read the nearby question before writing the {topic_title} answer relation."
                ),
                vec![answer_primary_ref, answer_secondary_ref],
            );
            tag_writer_pre_read_v3(
                &mut items,
                "near_shrink_same_subtask",
                variant_slug.as_str(),
                topic_slug.as_str(),
            );

            push_writer_pre_read_inspect_ambiguous(
                &mut items,
                run_id,
                &read_tools,
                &about,
                &format!("writer-v3-{slug}-answer-inspect-question"),
                &entry_answer,
                answer_primary_ref,
                answer_candidates.clone(),
                vec![
                    answer_primary_ref,
                    answer_secondary_ref,
                    previous_question.as_str(),
                ],
                &format!(
                    "Inspect the {topic_title} question before deciding whether the answer addresses or supersedes prior memory."
                ),
            );
            tag_writer_pre_read_v3(
                &mut items,
                "inspect_answer_candidate",
                variant_slug.as_str(),
                topic_slug.as_str(),
            );

            push_writer_pre_read_trace(
                &mut items,
                run_id,
                &read_tools,
                &about,
                &format!("writer-v3-{slug}-answer-trace-first"),
                &entry_answer,
                answer_primary_ref,
                answer_candidates.clone(),
                &format!(
                    "Trace the {topic_title} answer to the question because the relation needs proof before writing."
                ),
            );
            tag_writer_pre_read_v3(
                &mut items,
                "trace_first_answer_relation",
                variant_slug.as_str(),
                topic_slug.as_str(),
            );

            push_writer_pre_read_trace_continue(
                &mut items,
                run_id,
                &read_tools,
                &about,
                &format!("writer-v3-{slug}-answer-trace-continue"),
                &entry_answer,
                answer_primary_ref,
                answer_candidates.clone(),
                trace_cursor_answer,
                vec![
                    entry_answer.as_str(),
                    answer_primary_ref,
                    answer_secondary_ref,
                ],
                &format!(
                    "Continue the {topic_title} answer trace because the proof path is not complete."
                ),
            );
            tag_writer_pre_read_v3(
                &mut items,
                "trace_continue_answer_relation",
                variant_slug.as_str(),
                topic_slug.as_str(),
            );

            push_writer_pre_read_stop(
                &mut items,
                run_id,
                &read_tools,
                &about,
                &format!("writer-v3-{slug}-answer-stop-sufficient"),
                &entry_answer,
                answer_primary_ref,
                answer_candidates,
                vec![
                    entry_answer.as_str(),
                    answer_primary_ref,
                    answer_secondary_ref,
                ],
                vec![answer_primary_ref, answer_secondary_ref],
                &format!("sufficient_evidence_for_{topic_slug}_{variant_slug}_answer_relation"),
            );
            tag_writer_pre_read_v3(
                &mut items,
                "stop_after_answer_trace",
                variant_slug.as_str(),
                topic_slug.as_str(),
            );
        }
    }

    items
}

fn tag_writer_pre_read_v3(
    items: &mut [TrajectoryItem],
    synthetic_family: &str,
    variant: &str,
    topic: &str,
) {
    let Some(item) = items.last_mut() else {
        return;
    };
    item.task_family = "conformance.writer_pre_read.v3".to_string();
    if let Some(quality) = item.quality.as_object_mut() {
        quality.insert(
            "label_source".to_string(),
            json!("deterministic_synthetic_policy"),
        );
        quality.insert("synthetic_suite".to_string(), json!("writer-pre-read-v3"));
        quality.insert("synthetic_family".to_string(), json!(synthetic_family));
        quality.insert("synthetic_variant".to_string(), json!(variant));
        quality.insert("synthetic_topic".to_string(), json!(topic));
    }
}

fn writer_pre_read_v4_trajectories(run_id: &str) -> Vec<TrajectoryItem> {
    let mut items = writer_pre_read_v3_trajectories(run_id);
    let read_tools = kernel_operator_allowed_writer_pre_read_tools();

    let topics = fixture_or_panic(
        "writer_pre_read_topics.json",
        writer_pre_read_topics_from_json(include_str!(
            "../../fixtures/conformance/writer_pre_read_topics.json"
        )),
    );
    let variants = fixture_or_panic(
        "writer_pre_read_variants.json",
        writer_pre_read_variants_from_json(include_str!(
            "../../fixtures/conformance/writer_pre_read_variants.json"
        )),
    );

    let mut case_index = 0usize;
    for topic in &topics {
        for variant in &variants {
            case_index += 1;
            let topic_slug = topic.slug();
            let topic_title = topic.title();
            let question_hint = topic.question_hint();
            let answer_hint = topic.answer_hint();
            let variant_slug = variant.slug();
            let question_primary_role = variant.question_primary_role();
            let answer_secondary_role = variant.answer_secondary_role();
            let slug = format!("{topic_slug}-{variant_slug}-{case_index:02}");
            let about = format!("incident:{run_id}:writer-pre-read-v4:{slug}");
            let entry_question = format!("{about}:subtask:6:question");
            let entry_answer = format!("{about}:subtask:6:answer");
            let same_question = entry_question.clone();
            let previous_answer = format!("{about}:subtask:5:answer");
            let previous_question = format!("{about}:subtask:5:question");
            let older_answer = format!("{about}:subtask:4:answer");
            let older_question = format!("{about}:subtask:4:question");
            let trace_cursor_question = "16";
            let trace_cursor_answer = "16";
            let priority_offset = (case_index % 5) as u64;

            let question_primary_ref = if question_primary_role.selects_older_subtask_answer() {
                older_answer.as_str()
            } else {
                previous_answer.as_str()
            };
            let answer_primary_ref = same_question.as_str();
            let answer_secondary_ref = if answer_secondary_role.selects_previous_subtask_answer() {
                previous_answer.as_str()
            } else {
                older_question.as_str()
            };

            let question_candidates = writer_pre_read_candidate_details(vec![
                (
                    question_primary_ref,
                    question_primary_role.as_str(),
                    10 + priority_offset,
                    question_hint.as_str(),
                ),
                (
                    previous_question.as_str(),
                    "previous_subtask_question",
                    24 + priority_offset,
                    "question_may_refine_previous_question",
                ),
                (
                    older_answer.as_str(),
                    "older_subtask_answer",
                    35 + priority_offset,
                    "question_may_reopen_older_answer",
                ),
                (
                    older_question.as_str(),
                    "older_subtask_question",
                    50 + priority_offset,
                    "question_background_context_only",
                ),
            ]);
            let answer_candidates = writer_pre_read_candidate_details(vec![
                (
                    answer_primary_ref,
                    "same_subtask_question",
                    10 + priority_offset,
                    answer_hint.as_str(),
                ),
                (
                    answer_secondary_ref,
                    answer_secondary_role.as_str(),
                    18 + priority_offset,
                    "answer_may_reuse_prior_context",
                ),
                (
                    previous_question.as_str(),
                    "previous_subtask_question",
                    31 + priority_offset,
                    "answer_may_compare_prior_question",
                ),
                (
                    older_answer.as_str(),
                    "older_subtask_answer",
                    44 + priority_offset,
                    "answer_background_context_only",
                ),
            ]);

            push_writer_pre_read_trace(
                &mut items,
                run_id,
                &read_tools,
                &about,
                &format!("writer-v4-{slug}-question-trace-needed-after-inspect"),
                &entry_question,
                question_primary_ref,
                question_candidates.clone(),
                &format!(
                    "Trace the {topic_title} question relation because inspection did not yet prove the path."
                ),
            );
            tag_writer_pre_read_v4(
                &mut items,
                "trace_needed_after_inspect",
                variant_slug.as_str(),
                topic_slug.as_str(),
            );

            push_writer_pre_read_trace_continue(
                &mut items,
                run_id,
                &read_tools,
                &about,
                &format!("writer-v4-{slug}-question-trace-continue-needed"),
                &entry_question,
                question_primary_ref,
                question_candidates.clone(),
                trace_cursor_question,
                vec![
                    entry_question.as_str(),
                    question_primary_ref,
                    previous_question.as_str(),
                ],
                &format!(
                    "Continue the {topic_title} question trace because page metadata says more path evidence remains."
                ),
            );
            tag_writer_pre_read_v4(
                &mut items,
                "trace_continue_when_page_has_more",
                variant_slug.as_str(),
                topic_slug.as_str(),
            );

            push_writer_pre_read_stop_boundary(
                &mut items,
                run_id,
                &read_tools,
                &about,
                &format!("writer-v4-{slug}-question-stop-complete-trace-zero-budget"),
                &format!(
                    "Stop after the complete {topic_title} question trace; evidence is sufficient and no tool calls remain."
                ),
                &entry_question,
                question_primary_ref,
                question_candidates.clone(),
                vec![
                    entry_question.as_str(),
                    question_primary_ref,
                    previous_question.as_str(),
                ],
                vec![question_primary_ref, previous_question.as_str()],
            );
            tag_writer_pre_read_v4(
                &mut items,
                "stop_after_complete_trace_zero_budget",
                variant_slug.as_str(),
                topic_slug.as_str(),
            );

            push_writer_pre_read_trace(
                &mut items,
                run_id,
                &read_tools,
                &about,
                &format!("writer-v4-{slug}-answer-trace-needed-after-inspect"),
                &entry_answer,
                answer_primary_ref,
                answer_candidates.clone(),
                &format!(
                    "Trace the {topic_title} answer relation because inspection did not yet prove the path."
                ),
            );
            tag_writer_pre_read_v4(
                &mut items,
                "trace_needed_after_inspect",
                variant_slug.as_str(),
                topic_slug.as_str(),
            );

            push_writer_pre_read_trace_continue(
                &mut items,
                run_id,
                &read_tools,
                &about,
                &format!("writer-v4-{slug}-answer-trace-continue-needed"),
                &entry_answer,
                answer_primary_ref,
                answer_candidates.clone(),
                trace_cursor_answer,
                vec![
                    entry_answer.as_str(),
                    answer_primary_ref,
                    answer_secondary_ref,
                ],
                &format!(
                    "Continue the {topic_title} answer trace because page metadata says more path evidence remains."
                ),
            );
            tag_writer_pre_read_v4(
                &mut items,
                "trace_continue_when_page_has_more",
                variant_slug.as_str(),
                topic_slug.as_str(),
            );

            push_writer_pre_read_stop_boundary(
                &mut items,
                run_id,
                &read_tools,
                &about,
                &format!("writer-v4-{slug}-answer-stop-complete-trace-zero-budget"),
                &format!(
                    "Stop after the complete {topic_title} answer trace; evidence is sufficient and no tool calls remain."
                ),
                &entry_answer,
                answer_primary_ref,
                answer_candidates,
                vec![
                    entry_answer.as_str(),
                    answer_primary_ref,
                    answer_secondary_ref,
                ],
                vec![answer_primary_ref, answer_secondary_ref],
            );
            tag_writer_pre_read_v4(
                &mut items,
                "stop_after_complete_trace_zero_budget",
                variant_slug.as_str(),
                topic_slug.as_str(),
            );
        }
    }

    items
}

fn tag_writer_pre_read_v4(
    items: &mut [TrajectoryItem],
    synthetic_family: &str,
    variant: &str,
    topic: &str,
) {
    let Some(item) = items.last_mut() else {
        return;
    };
    item.task_family = "conformance.writer_pre_read.v4".to_string();
    if let Some(quality) = item.quality.as_object_mut() {
        quality.insert(
            "label_source".to_string(),
            json!("deterministic_synthetic_policy"),
        );
        quality.insert("synthetic_suite".to_string(), json!("writer-pre-read-v4"));
        quality.insert("synthetic_family".to_string(), json!(synthetic_family));
        quality.insert("synthetic_variant".to_string(), json!(variant));
        quality.insert("synthetic_topic".to_string(), json!(topic));
        quality.insert("hard_negative_family".to_string(), json!("stop_vs_trace"));
    }
}

fn writer_exec_v1_trajectories(run_id: &str) -> Vec<TrajectoryItem> {
    let mut items = Vec::new();
    let topics = fixture_or_panic(
        "writer_exec_topics.json",
        writer_exec_topics_from_json(include_str!(
            "../../fixtures/conformance/writer_exec_topics.json"
        )),
    );

    for (index, topic_fixture) in topics.iter().enumerate() {
        let topic_slug = topic_fixture.slug();
        let topic = topic_fixture.title();
        let signal = topic_fixture.signal();
        let decision = topic_fixture.decision();
        let topic_slug_text = topic_slug.as_str();
        let topic_text = topic.as_str();
        let signal_text = signal.as_str();
        let decision_text = decision.as_str();
        let sequence_base = ((index + 1) * 10) as u64;
        let about = format!("incident:{run_id}:writer-exec-v1:{topic_slug}");
        let question_ref = format!("{about}:question:root");
        let observation_ref = format!("{about}:observation:primary-signal");
        let stale_ref = format!("{about}:hypothesis:stale-path");
        let state_ref = format!("{about}:state:policy");
        let prior_decision_ref = format!("{about}:decision:previous-step");
        let known_refs = json!([
            question_ref,
            observation_ref,
            stale_ref,
            state_ref,
            prior_decision_ref
        ]);
        let read_context = json!({
            "inspected_refs": [observation_ref, state_ref],
            "temporal_refs": [stale_ref, prior_decision_ref],
            "ask_refs": [question_ref]
        });

        let rich_args = writer_exec_write_memory_arguments(
            &about,
            topic_slug_text,
            sequence_base,
            "record_decision",
            "decision",
            &format!("For {topic}, {decision}."),
            &format!("Visible evidence says: {signal}."),
            &observation_ref,
            "chosen_because",
            "causal",
            &format!("The new decision directly addresses the observed {topic} signal."),
            signal_text,
            "high",
            read_context.clone(),
            Some(json!({
                "from": format!("The process still considered the stale {topic} path."),
                "to": format!("The process now selects the {topic} decision."),
                "why": format!("The inspected signal supports the {topic} decision."),
                "evidence": signal_text
            })),
            "rich-chosen-because",
        );
        push_writer_exec(
            &mut items,
            run_id,
            &about,
            &format!("writer-exec-v1-{topic_slug}-write-rich-chosen-because"),
            &format!("Execute the prepared rich {topic} memory write."),
            writer_exec_positive_state(
                &about,
                topic_slug_text,
                topic_text,
                &known_refs,
                &read_context,
                rich_args.clone(),
                "execute_prepared_write",
            ),
            tool_call("kernel_write_memory", rich_args),
            json!({
                "success": true,
                "dry_run": true,
                "compiled_to": "kernel_ingest",
                "write_attempted": true
            }),
            writer_exec_quality(
                topic_slug_text,
                "write_rich_chosen_because",
                json!({
                    "write_relation_quality": "rich",
                    "read_context_proof": true,
                    "prepared_write": true
                }),
            ),
        );

        let contradicts_args = writer_exec_write_memory_arguments(
            &about,
            topic_slug_text,
            sequence_base + 1,
            "record_observation",
            "observation",
            &format!("The {topic} signal contradicts the stale path."),
            signal_text,
            &stale_ref,
            "contradicts",
            "evidential",
            &format!("The observed {topic} signal conflicts with the stale hypothesis."),
            signal_text,
            "high",
            json!({
                "inspected_refs": [stale_ref, observation_ref],
                "temporal_refs": [prior_decision_ref],
                "ask_refs": [question_ref]
            }),
            None,
            "rich-contradicts",
        );
        push_writer_exec(
            &mut items,
            run_id,
            &about,
            &format!("writer-exec-v1-{topic_slug}-write-rich-contradicts"),
            &format!("Execute the prepared contradiction write for {topic}."),
            writer_exec_positive_state(
                &about,
                topic_slug_text,
                topic_text,
                &known_refs,
                &read_context,
                contradicts_args.clone(),
                "execute_prepared_write",
            ),
            tool_call("kernel_write_memory", contradicts_args),
            json!({ "success": true, "dry_run": true, "compiled_to": "kernel_ingest" }),
            writer_exec_quality(
                topic_slug_text,
                "write_rich_contradicts",
                json!({
                    "write_relation_quality": "rich",
                    "read_context_proof": true,
                    "prepared_write": true
                }),
            ),
        );

        let follows_args = writer_exec_write_memory_arguments(
            &about,
            topic_slug_text,
            sequence_base + 2,
            "record_turn",
            "turn",
            &format!("Record the next {topic} status note."),
            &format!("The note only continues the {topic} process timeline."),
            &prior_decision_ref,
            "follows",
            "procedural",
            &format!("The note follows the prior {topic} decision in process order."),
            "No stronger causal or evidential relation is visible.",
            "medium",
            json!({
                "temporal_refs": [prior_decision_ref],
                "inspected_refs": [observation_ref]
            }),
            None,
            "anemic-follows",
        );
        push_writer_exec(
            &mut items,
            run_id,
            &about,
            &format!("writer-exec-v1-{topic_slug}-write-anemic-follows"),
            &format!("Execute the prepared anemic fallback write for {topic}."),
            writer_exec_positive_state(
                &about,
                topic_slug_text,
                topic_text,
                &known_refs,
                &json!({ "temporal_refs": [prior_decision_ref], "inspected_refs": [observation_ref] }),
                follows_args.clone(),
                "execute_anemic_fallback",
            ),
            tool_call("kernel_write_memory", follows_args),
            json!({ "success": true, "dry_run": true, "compiled_to": "kernel_ingest" }),
            writer_exec_quality(
                topic_slug_text,
                "write_anemic_follows",
                json!({
                    "write_relation_quality": "anemic",
                    "read_context_proof": true,
                    "prepared_write": true,
                    "anemic_fallback": true
                }),
            ),
        );

        let updates_args = writer_exec_write_memory_arguments(
            &about,
            topic_slug_text,
            sequence_base + 3,
            "record_delta",
            "semantic_delta",
            &format!("Update the {topic} policy state."),
            decision_text,
            &state_ref,
            "updates_state",
            "causal",
            &format!("The prepared delta updates the {topic} state."),
            decision_text,
            "high",
            json!({
                "inspected_refs": [state_ref, observation_ref],
                "temporal_refs": [prior_decision_ref]
            }),
            Some(json!({
                "from": format!("The {topic} state still allowed the stale path."),
                "to": format!("The {topic} state now records the selected decision."),
                "why": format!("The writer prepared a state update for {topic}."),
                "evidence": decision_text
            })),
            "rich-updates-state",
        );
        push_writer_exec(
            &mut items,
            run_id,
            &about,
            &format!("writer-exec-v1-{topic_slug}-write-updates-state"),
            &format!("Execute the prepared state update write for {topic}."),
            writer_exec_positive_state(
                &about,
                topic_slug_text,
                topic_text,
                &known_refs,
                &read_context,
                updates_args.clone(),
                "execute_prepared_write",
            ),
            tool_call("kernel_write_memory", updates_args),
            json!({ "success": true, "dry_run": true, "compiled_to": "kernel_ingest" }),
            writer_exec_quality(
                topic_slug_text,
                "write_updates_state",
                json!({
                    "write_relation_quality": "rich",
                    "read_context_proof": true,
                    "prepared_write": true,
                    "semantic_delta": true
                }),
            ),
        );

        let ingest_single = writer_exec_ingest_arguments(
            &about,
            topic_slug_text,
            sequence_base + 4,
            topic_text,
            signal_text,
            decision_text,
            false,
        );
        push_writer_exec(
            &mut items,
            run_id,
            &about,
            &format!("writer-exec-v1-{topic_slug}-ingest-single"),
            &format!("Execute canonical ingest for a complete {topic} payload."),
            writer_exec_canonical_state(
                &about,
                topic_slug_text,
                topic_text,
                &known_refs,
                ingest_single.clone(),
                "execute_canonical_ingest",
            ),
            tool_call("kernel_ingest", ingest_single),
            json!({ "success": true, "dry_run": true, "entries": 1 }),
            writer_exec_quality(
                topic_slug_text,
                "ingest_single",
                json!({
                    "canonical_write": true,
                    "canonical_payload_ready": true
                }),
            ),
        );

        let ingest_multi = writer_exec_ingest_arguments(
            &about,
            topic_slug_text,
            sequence_base + 5,
            topic_text,
            signal_text,
            decision_text,
            true,
        );
        push_writer_exec(
            &mut items,
            run_id,
            &about,
            &format!("writer-exec-v1-{topic_slug}-ingest-multidimensional"),
            &format!("Execute canonical ingest for a multidimensional {topic} payload."),
            writer_exec_canonical_state(
                &about,
                topic_slug_text,
                topic_text,
                &known_refs,
                ingest_multi.clone(),
                "execute_canonical_ingest",
            ),
            tool_call("kernel_ingest", ingest_multi),
            json!({ "success": true, "dry_run": true, "entries": 2 }),
            writer_exec_quality(
                topic_slug_text,
                "ingest_multidimensional",
                json!({
                    "canonical_write": true,
                    "canonical_payload_ready": true,
                    "multidimensional_payload": true
                }),
            ),
        );

        push_writer_exec_stop(
            &mut items,
            run_id,
            &about,
            topic_slug_text,
            topic_text,
            &known_refs,
            "missing_prepared_payload",
            "missing_prepared_write_payload",
            json!({
                "draft_write": {
                    "intent": "record_decision",
                    "current": {
                        "kind": "decision",
                        "summary": format!("For {topic}, {decision}."),
                        "evidence": signal_text
                    },
                    "execution_requirement": "do not reconstruct write payload; stop until prepared_arguments are visible"
                }
            }),
            Vec::new(),
        );

        push_writer_exec_stop(
            &mut items,
            run_id,
            &about,
            topic_slug_text,
            topic_text,
            &known_refs,
            "missing_read_context_proof",
            "write_requires_read_context_proof",
            json!({
                "draft_write": {
                    "intent": "record_decision",
                    "prepared_arguments": writer_exec_without_read_context_proof(
                        writer_exec_write_memory_arguments(
                            &about,
                            topic_slug_text,
                            sequence_base + 6,
                            "record_decision",
                            "decision",
                            &format!("For {topic}, {decision}."),
                            signal_text,
                            &observation_ref,
                            "chosen_because",
                            "causal",
                            &format!("The decision claims to depend on the {topic} observation."),
                            signal_text,
                            "high",
                            json!({ "inspected_refs": [], "temporal_refs": [] }),
                            None,
                            "invalid-missing-proof",
                        )
                    ),
                    "execution_requirement": "rich relation targets must be proven by read_context"
                },
                "read_context": { "inspected_refs": [], "temporal_refs": [] },
                "candidate_refs": [observation_ref]
            }),
            vec![observation_ref.as_str()],
        );

        push_writer_exec_stop(
            &mut items,
            run_id,
            &about,
            topic_slug_text,
            topic_text,
            &known_refs,
            "invalid_relation",
            "invalid_write_relation",
            json!({
                "draft_write": {
                    "intent": "record_decision",
                    "prepared_arguments": writer_exec_invalid_relation(
                        writer_exec_write_memory_arguments(
                            &about,
                            topic_slug_text,
                            sequence_base + 7,
                            "record_decision",
                            "decision",
                            &format!("For {topic}, {decision}."),
                            signal_text,
                            &observation_ref,
                            "chosen_because",
                            "causal",
                            "This will be replaced by an unsupported relation.",
                            signal_text,
                            "medium",
                            read_context.clone(),
                            None,
                            "invalid-relation",
                        )
                    ),
                    "execution_requirement": "unsupported or vague relations must not be sent to kernel_write_memory"
                },
                "read_context": read_context,
                "candidate_refs": [observation_ref]
            }),
            vec![observation_ref.as_str()],
        );

        push_writer_exec_stop(
            &mut items,
            run_id,
            &about,
            topic_slug_text,
            topic_text,
            &known_refs,
            "about_scope_mismatch",
            "write_about_scope_mismatch",
            json!({
                "draft_write": {
                    "intent": "record_decision",
                    "prepared_arguments": writer_exec_about_mismatch(
                        writer_exec_write_memory_arguments(
                            &about,
                            topic_slug_text,
                            sequence_base + 8,
                            "record_decision",
                            "decision",
                            &format!("For {topic}, {decision}."),
                            signal_text,
                            &observation_ref,
                            "chosen_because",
                            "causal",
                            "The target about will be replaced to simulate a bad prepared payload.",
                            signal_text,
                            "medium",
                            json!({ "inspected_refs": [observation_ref] }),
                            None,
                            "about-mismatch",
                        )
                    ),
                    "execution_requirement": "prepared_arguments.about must equal the trajectory about"
                },
                "read_context": { "inspected_refs": [observation_ref] },
                "candidate_refs": [observation_ref]
            }),
            vec![observation_ref.as_str()],
        );

        push_writer_exec_stop(
            &mut items,
            run_id,
            &about,
            topic_slug_text,
            topic_text,
            &known_refs,
            "incomplete_canonical_payload",
            "incomplete_canonical_payload",
            json!({
                "canonical_payload_ready": true,
                "canonical_payload": writer_exec_incomplete_canonical_payload(
                    writer_exec_ingest_arguments(
                        &about,
                        topic_slug_text,
                        sequence_base + 9,
                        topic_text,
                        signal_text,
                        decision_text,
                        false,
                    )
                ),
                "execution_requirement": "canonical ingest requires a memory.dimensions array, entries with coordinates, idempotency key, and explicit dry_run; optional relations, evidence, and provenance are not reconstructed"
            }),
            Vec::new(),
        );

        let duplicate_args = writer_exec_write_memory_arguments(
            &about,
            topic_slug_text,
            sequence_base + 10,
            "record_decision",
            "decision",
            &format!("For {topic}, {decision}."),
            signal_text,
            &observation_ref,
            "chosen_because",
            "causal",
            &format!("The duplicate write repeats the prepared {topic} decision."),
            signal_text,
            "high",
            json!({ "inspected_refs": [observation_ref] }),
            None,
            "duplicate",
        );
        let duplicate_key = duplicate_args
            .get("idempotency_key")
            .and_then(Value::as_str)
            .unwrap_or("missing")
            .to_string();
        push_writer_exec_stop(
            &mut items,
            run_id,
            &about,
            topic_slug_text,
            topic_text,
            &known_refs,
            "duplicate_idempotency",
            "duplicate_idempotency_key",
            json!({
                "draft_write": {
                    "intent": "record_decision",
                    "prepared_arguments": duplicate_args,
                    "execution_requirement": "do not re-send a prepared write whose idempotency key already completed"
                },
                "existing_idempotency_keys": [duplicate_key],
                "read_context": { "inspected_refs": [observation_ref] },
                "candidate_refs": [observation_ref]
            }),
            vec![observation_ref.as_str()],
        );

        push_writer_exec_stop(
            &mut items,
            run_id,
            &about,
            topic_slug_text,
            topic_text,
            &known_refs,
            "strict_required",
            "strict_write_required",
            json!({
                "draft_write": {
                    "intent": "record_decision",
                    "prepared_arguments": writer_exec_non_strict_options(
                        writer_exec_write_memory_arguments(
                            &about,
                            topic_slug_text,
                            sequence_base + 11,
                            "record_decision",
                            "decision",
                            &format!("For {topic}, {decision}."),
                            signal_text,
                            &observation_ref,
                            "chosen_because",
                            "causal",
                            &format!("The prepared {topic} write disables strict mode."),
                            signal_text,
                            "medium",
                            json!({ "inspected_refs": [observation_ref] }),
                            None,
                            "non-strict",
                        )
                    ),
                    "execution_requirement": "operator writes must keep options.strict=true"
                },
                "read_context": { "inspected_refs": [observation_ref] },
                "candidate_refs": [observation_ref]
            }),
            vec![observation_ref.as_str()],
        );
    }

    items
}

#[allow(clippy::too_many_arguments)]
fn push_writer_exec(
    items: &mut Vec<TrajectoryItem>,
    run_id: &str,
    about: &str,
    step_id: &str,
    goal: &str,
    visible_state: Value,
    target_action: Value,
    observed_outcome: Value,
    quality: Value,
) {
    push(
        items,
        item(
            run_id,
            "conformance.writer_exec.v1",
            "write",
            about,
            step_id,
            goal,
            visible_state,
            kernel_operator_allowed_write_tools(),
            target_action,
            observed_outcome,
            quality,
        ),
    );
}

#[allow(clippy::too_many_arguments)]
fn push_writer_exec_stop(
    items: &mut Vec<TrajectoryItem>,
    run_id: &str,
    about: &str,
    topic_slug: &str,
    topic: &str,
    known_refs: &Value,
    synthetic_family: &str,
    reason: &str,
    extra_state: Value,
    final_refs: Vec<&str>,
) {
    let mut state = json!({
        "current_ref": format!("{about}:draft:{synthetic_family}"),
        "known_refs": known_refs,
        "last_tool": "writer_prepared_payload_audit",
        "last_observed_refs": [],
        "remaining_budget": budget(1),
        "requested_stop": requested_stop("evidence_or_unknown", final_refs.clone(), reason),
        "operator_state": {
            "decision": "stop_write_execution",
            "topic": topic,
            "failfast_reason": reason
        }
    });
    merge_object(&mut state, extra_state);
    push_writer_exec(
        items,
        run_id,
        about,
        &format!("writer-exec-v1-{topic_slug}-stop-{synthetic_family}"),
        &format!("Fail fast for {topic} write execution: {reason}."),
        state,
        stop_action("evidence_or_unknown", final_refs, reason),
        json!({
            "success": true,
            "write_attempted": false,
            "failfast": true,
            "reason": reason
        }),
        writer_exec_quality(
            topic_slug,
            synthetic_family,
            json!({
                "expected_failfast": true,
                "failfast_reason": reason
            }),
        ),
    );
}

fn merge_object(target: &mut Value, extra: Value) {
    let Some(target_object) = target.as_object_mut() else {
        return;
    };
    let Value::Object(extra_object) = extra else {
        return;
    };
    for (key, value) in extra_object {
        target_object.insert(key, value);
    }
}

fn writer_exec_positive_state(
    about: &str,
    topic_slug: &str,
    topic: &str,
    known_refs: &Value,
    read_context: &Value,
    prepared_arguments: Value,
    decision: &str,
) -> Value {
    json!({
        "current_ref": format!("{about}:draft:{topic_slug}:prepared-write"),
        "known_refs": known_refs,
        "last_tool": "writer_pre_read_complete",
        "last_observed_refs": read_context_refs_for_state(read_context),
        "candidate_refs": read_context_refs_for_state(read_context),
        "read_context": read_context,
        "draft_write": {
            "intent": prepared_arguments.get("intent").cloned().unwrap_or(json!("record_memory")),
            "prepared_arguments": prepared_arguments,
            "execution_requirement": "copy prepared_arguments exactly; do not reconstruct text or relations"
        },
        "remaining_budget": budget(1),
        "operator_state": {
            "decision": decision,
            "topic": topic,
            "payload_status": "prepared",
            "write_contract": "strict"
        }
    })
}

fn writer_exec_canonical_state(
    about: &str,
    topic_slug: &str,
    topic: &str,
    known_refs: &Value,
    canonical_payload: Value,
    decision: &str,
) -> Value {
    json!({
        "current_ref": format!("{about}:entry:{topic_slug}:canonical"),
        "known_refs": known_refs,
        "last_tool": "writer_prepared_payload_audit",
        "last_observed_refs": [],
        "canonical_payload_ready": true,
        "canonical_payload": canonical_payload,
        "execution_requirement": "copy canonical_payload exactly into kernel_ingest; do not rewrite typed memory",
        "remaining_budget": budget(1),
        "operator_state": {
            "decision": decision,
            "topic": topic,
            "payload_status": "canonical",
            "write_contract": "kernel_ingest"
        }
    })
}

fn read_context_refs_for_state(read_context: &Value) -> Value {
    let mut refs = Vec::new();
    for field in ["inspected_refs", "temporal_refs", "wake_refs", "ask_refs"] {
        if let Some(values) = read_context.get(field).and_then(Value::as_array) {
            for value in values {
                if let Some(value) = value.as_str() {
                    refs.push(value.to_string());
                }
            }
        }
    }
    json!(refs)
}

fn writer_exec_quality(topic_slug: &str, synthetic_family: &str, extra: Value) -> Value {
    let mut quality = json!({
        "bounded": true,
        "contract_expected": true,
        "label_source": "deterministic_synthetic_policy",
        "synthetic_suite": "writer-exec-v1",
        "synthetic_family": synthetic_family,
        "synthetic_topic": topic_slug
    });
    merge_object(&mut quality, extra);
    quality
}

#[allow(clippy::too_many_arguments)]
fn writer_exec_write_memory_arguments(
    about: &str,
    topic_slug: &str,
    sequence: u64,
    intent: &str,
    current_kind: &str,
    current_summary: &str,
    current_evidence: &str,
    target_ref: &str,
    rel: &str,
    class: &str,
    why: &str,
    evidence: &str,
    confidence: &str,
    read_context: Value,
    semantic_delta: Option<Value>,
    id_suffix: &str,
) -> Value {
    let mut arguments = json!({
        "about": about,
        "intent": intent,
        "actor": "operator:writer-exec-v1",
        "observed_at": format!("2026-05-16T10:{:02}:00Z", sequence % 60),
        "scope": {
            "task": about,
            "process": format!("{about}:resolution"),
            "episode": format!("{about}:episode:writer")
        },
        "current": {
            "kind": current_kind,
            "summary": current_summary,
            "evidence": current_evidence,
            "ref": format!("{about}:entry:{topic_slug}:{id_suffix}")
        },
        "connect_to": [
            {
                "ref": target_ref,
                "rel": rel,
                "class": class,
                "why": why,
                "evidence": evidence,
                "confidence": confidence
            }
        ],
        "read_context": read_context,
        "idempotency_key": format!("{about}:writer-exec-v1:{topic_slug}:{id_suffix}"),
        "options": {
            "dry_run": true,
            "strict": true,
            "sequence": sequence
        },
        "source_kind": "agent"
    });
    if let Some(semantic_delta) = semantic_delta {
        arguments["semantic_delta"] = semantic_delta;
    }
    arguments
}

fn writer_exec_ingest_arguments(
    about: &str,
    topic_slug: &str,
    sequence: u64,
    topic: &str,
    signal: &str,
    decision: &str,
    multidimensional: bool,
) -> Value {
    let observation_entry = format!("{about}:entry:{topic_slug}:observation");
    let decision_entry = format!("{about}:entry:{topic_slug}:decision");
    let mut dimensions = vec![json!({
        "id": about,
        "kind": "task",
        "title": format!("{topic} task")
    })];
    let mut observation_coordinates = vec![json!({
        "dimension": "task",
        "scope_id": about,
        "sequence": sequence,
        "observed_at": format!("2026-05-16T11:{:02}:00Z", sequence % 60)
    })];
    let mut decision_coordinates = vec![json!({
        "dimension": "task",
        "scope_id": about,
        "sequence": sequence + 1,
        "observed_at": format!("2026-05-16T11:{:02}:30Z", sequence % 60)
    })];
    if multidimensional {
        dimensions.push(json!({
            "id": "agent:writer",
            "kind": "agent",
            "title": "Writer agent"
        }));
        observation_coordinates.push(json!({
            "dimension": "agent",
            "scope_id": "agent:writer",
            "sequence": 1,
            "observed_at": format!("2026-05-16T11:{:02}:00Z", sequence % 60)
        }));
        decision_coordinates.push(json!({
            "dimension": "agent",
            "scope_id": "agent:writer",
            "sequence": 2,
            "observed_at": format!("2026-05-16T11:{:02}:30Z", sequence % 60)
        }));
    }

    json!({
        "about": about,
        "idempotency_key": format!("{about}:writer-exec-v1:{topic_slug}:canonical:{}", if multidimensional { "multi" } else { "single" }),
        "dry_run": true,
        "memory": {
            "dimensions": dimensions,
            "entries": [
                {
                    "id": observation_entry,
                    "kind": "observation",
                    "text": signal,
                    "coordinates": observation_coordinates
                },
                {
                    "id": decision_entry,
                    "kind": "decision",
                    "text": decision,
                    "coordinates": decision_coordinates
                }
            ],
            "relations": [
                {
                    "from": decision_entry,
                    "to": observation_entry,
                    "rel": "chosen_because",
                    "class": "causal",
                    "why": format!("The {topic} decision is chosen because the observation was present."),
                    "evidence": signal,
                    "confidence": "high",
                    "sequence": 1
                }
            ],
            "evidence": [
                {
                    "id": format!("{about}:evidence:{topic_slug}:signal"),
                    "supports": [observation_entry, decision_entry],
                    "text": signal,
                    "source": "synthetic_conformance",
                    "time": format!("2026-05-16T11:{:02}:00Z", sequence % 60)
                }
            ]
        },
        "provenance": {
            "source_kind": "agent",
            "source_agent": "operator:writer-exec-v1",
            "observed_at": format!("2026-05-16T11:{:02}:00Z", sequence % 60),
            "correlation_id": format!("writer-exec-v1:{topic_slug}"),
            "causation_id": format!("writer-exec-v1:{topic_slug}:canonical")
        }
    })
}

fn writer_exec_without_read_context_proof(mut arguments: Value) -> Value {
    arguments["read_context"] = json!({
        "inspected_refs": [],
        "temporal_refs": []
    });
    arguments
}

fn writer_exec_invalid_relation(mut arguments: Value) -> Value {
    if let Some(link) = arguments
        .get_mut("connect_to")
        .and_then(Value::as_array_mut)
        .and_then(|links| links.first_mut())
    {
        link["rel"] = json!("related_to");
        link["class"] = json!("associative");
        link["why"] = json!("The relation is intentionally vague and unsupported.");
        link["evidence"] = json!("No precise relation was prepared.");
    }
    arguments
}

fn writer_exec_about_mismatch(mut arguments: Value) -> Value {
    arguments["about"] = json!("incident:writer-exec-v1:wrong-about");
    arguments
}

fn writer_exec_incomplete_canonical_payload(mut arguments: Value) -> Value {
    if let Some(entry) = arguments
        .pointer_mut("/memory/entries/0")
        .and_then(Value::as_object_mut)
    {
        entry.remove("coordinates");
    }
    arguments
}

fn writer_exec_non_strict_options(mut arguments: Value) -> Value {
    arguments["options"]["strict"] = json!(false);
    arguments
}

fn push_training_corpus_variants(
    items: &mut Vec<TrajectoryItem>,
    run_id: &str,
    read_tools: &[String],
    full_tools: &[String],
) {
    let about = "incident:mobile-login";
    let sibling_about = "incident:payments";
    let current_ref = "incident:mobile-login:question:login-failure";
    let prior_observation_ref = "incident:mobile-login:observation:401-refresh-race";
    let prior_timeout_ref = "incident:mobile-login:hypothesis:network-timeout";
    let final_decision_ref = "incident:mobile-login:decision:refresh-retry";
    let stale_decision_ref = "incident:mobile-login:decision:widen-timeout";
    let race_confirmed_ref = "incident:mobile-login:entry:observation:refresh-race-confirmed";
    let policy_ref = "incident:mobile-login:state:token-refresh-policy";
    let constraint_ref = "incident:mobile-login:constraint:no-timeout-widening";
    let reader_refs = [
        current_ref,
        prior_observation_ref,
        prior_timeout_ref,
        final_decision_ref,
        stale_decision_ref,
        race_confirmed_ref,
        policy_ref,
        constraint_ref,
    ];

    for (step_id, dimensions, question, answer_policy) in [
        (
            "ask-about-list-cross-product",
            json!({ "mode": "all", "scope": "abouts", "abouts": [about, sibling_about] }),
            "Which related incidents mention token refresh failures?",
            "evidence_or_unknown",
        ),
        (
            "ask-except-discarded-attempts",
            json!({ "mode": "except", "scope": "current_about", "exclude": ["attempt:discarded", "scratch"] }),
            "What evidence remains after excluding discarded attempts?",
            "evidence_or_unknown",
        ),
        (
            "ask-only-agent-and-decision-dimensions",
            json!({ "mode": "only", "scope": "current_about", "include": ["agent:triage", "decision"] }),
            "Which agent observations support the refresh retry decision?",
            "show_conflicts",
        ),
    ] {
        let goal = format!("Ask using the requested dimension scope: {question}");
        push(
            items,
            item(
                run_id,
                "conformance.read.ask",
                "read",
                about,
                step_id,
                &goal,
                json!({
                    "current_ref": current_ref,
                    "known_refs": reader_refs,
                    "last_tool": "kernel_near",
                    "last_observed_refs": [prior_observation_ref, final_decision_ref],
                    "remaining_budget": budget(3),
                    "requested_ask": requested_ask(
                        question,
                        answer_policy,
                        dimensions.clone(),
                        2600,
                        2
                    ),
                    "operator_state": {
                        "decision": "ask_for_context",
                        "dimensions_are_explicit": true
                    }
                }),
                read_tools.to_vec(),
                tool_call(
                    "kernel_ask",
                    json!({
                        "about": about,
                        "answer_policy": answer_policy,
                        "dimensions": dimensions,
                        "question": question,
                        "budget": { "tokens": 2600 },
                        "depth": 2
                    }),
                ),
                json!({ "success": true, "observed_refs": [prior_observation_ref, final_decision_ref] }),
                json!({ "bounded": true, "contract_expected": true, "variant": "dimension_scope" }),
            ),
        );
    }

    for (step_id, invalid_dimensions, reason) in [
        (
            "stop-invalid-dimensions-only-without-include",
            json!({ "mode": "only", "scope": "current_about" }),
            "invalid_dimension_selection_only_requires_include",
        ),
        (
            "stop-invalid-dimensions-except-without-exclude",
            json!({ "mode": "except", "scope": "current_about" }),
            "invalid_dimension_selection_except_requires_exclude",
        ),
        (
            "stop-invalid-dimensions-abouts-empty",
            json!({ "mode": "all", "scope": "abouts", "abouts": [] }),
            "invalid_dimension_selection_abouts_requires_list",
        ),
        (
            "stop-invalid-dimensions-all-abouts-with-list",
            json!({ "mode": "all", "scope": "all_abouts", "abouts": [about] }),
            "invalid_dimension_selection_all_abouts_must_not_set_abouts",
        ),
    ] {
        push(
            items,
            item(
                run_id,
                "conformance.read.failfast",
                "read",
                about,
                step_id,
                "Fail fast instead of emitting an invalid dimension selection.",
                json!({
                    "current_ref": current_ref,
                    "known_refs": [current_ref],
                    "last_tool": "kernel_ask",
                    "last_observed_refs": [],
                    "proposed_tool": "kernel_ask",
                    "proposed_arguments": {
                        "about": about,
                        "dimensions": invalid_dimensions,
                        "question": "What context is relevant?"
                    },
                    "remaining_budget": budget(2),
                    "requested_stop": requested_stop(
                        "evidence_or_unknown",
                        Vec::new(),
                        reason
                    ),
                    "operator_state": {
                        "decision": "stop_invalid_arguments",
                        "why": reason
                    }
                }),
                read_tools.to_vec(),
                stop_action("evidence_or_unknown", Vec::new(), reason),
                json!({ "success": true, "invalid_tool_call_emitted": false }),
                json!({
                    "bounded": true,
                    "contract_expected": true,
                    "expected_failfast": true,
                    "negative_example": "dimension_selection"
                }),
            ),
        );
    }

    for (tool, cursor_key, cursor, step_id, decision, limit, window) in [
        (
            "kernel_goto",
            "at",
            json!({ "ref": race_confirmed_ref }),
            "goto-by-ref-confirmed-observation",
            "goto_known_ref",
            json!({ "entries": 8, "tokens": 1800 }),
            json!({ "before_entries": 2, "after_entries": 2 }),
        ),
        (
            "kernel_goto",
            "at",
            json!({ "time": "2026-05-06T10:06:00Z" }),
            "goto-by-time-confirmation-snapshot",
            "goto_known_time",
            json!({ "entries": 10, "tokens": 2200 }),
            json!({ "before_entries": 4, "after_entries": 2 }),
        ),
        (
            "kernel_goto",
            "at",
            json!({ "sequence": 11 }),
            "goto-by-sequence-final-remediation",
            "goto_known_sequence",
            json!({ "entries": 10, "tokens": 2200 }),
            json!({ "before_entries": 4, "after_entries": 2 }),
        ),
        (
            "kernel_rewind",
            "from",
            json!({ "ref": race_confirmed_ref }),
            "rewind-from-confirmed-observation",
            "rewind_find_prior_cause",
            json!({ "entries": 14, "tokens": 2400 }),
            json!({ "before_entries": 8, "after_entries": 0 }),
        ),
        (
            "kernel_rewind",
            "from",
            json!({ "time": "2026-05-06T10:07:00Z" }),
            "rewind-from-time-before-final-decision",
            "rewind_find_previous_state",
            json!({ "entries": 14, "tokens": 2400 }),
            json!({ "before_entries": 8, "after_entries": 0 }),
        ),
        (
            "kernel_rewind",
            "from",
            json!({ "sequence": 11 }),
            "rewind-from-sequence-before-remediation",
            "rewind_find_previous_sequence",
            json!({ "entries": 14, "tokens": 2400 }),
            json!({ "before_entries": 8, "after_entries": 0 }),
        ),
        (
            "kernel_forward",
            "from",
            json!({ "ref": stale_decision_ref }),
            "forward-from-stale-decision",
            "forward_find_replacement",
            json!({ "entries": 14, "tokens": 2400 }),
            json!({ "before_entries": 0, "after_entries": 8 }),
        ),
        (
            "kernel_forward",
            "from",
            json!({ "time": "2026-05-06T10:03:00Z" }),
            "forward-from-time-find-confirmation",
            "forward_find_later_update",
            json!({ "entries": 14, "tokens": 2400 }),
            json!({ "before_entries": 0, "after_entries": 8 }),
        ),
        (
            "kernel_forward",
            "from",
            json!({ "sequence": 7 }),
            "forward-from-sequence-find-decision",
            "forward_find_next_decision",
            json!({ "entries": 14, "tokens": 2400 }),
            json!({ "before_entries": 0, "after_entries": 8 }),
        ),
    ] {
        push(
            items,
            item(
                run_id,
                "conformance.read.temporal",
                "read",
                about,
                step_id,
                "Choose the temporal tool and cursor mode that matches the requested movement.",
                json!({
                    "current_ref": final_decision_ref,
                    "known_refs": reader_refs,
                    "last_tool": "kernel_trace",
                    "last_observed_refs": [prior_observation_ref, final_decision_ref],
                    "requested_move": requested_move(tool, cursor_key, cursor.clone()),
                    "requested_scope": json!({ "mode": "all", "scope": "current_about" }),
                    "requested_bounds": requested_bounds(limit.clone(), window.clone()),
                    "remaining_budget": budget(3),
                    "operator_state": {
                        "decision": decision,
                        "expected_tool": tool,
                        "cursor_key": cursor_key
                    }
                }),
                read_tools.to_vec(),
                temporal_call(
                    tool,
                    cursor_key,
                    cursor,
                    json!({ "mode": "all", "scope": "current_about" }),
                    limit,
                    window,
                ),
                json!({ "success": true, "observed_refs": [prior_observation_ref, final_decision_ref, race_confirmed_ref] }),
                json!({
                    "bounded": true,
                    "contract_expected": true,
                    "policy": "temporal_direction",
                    "expected_tool": tool
                }),
            ),
        );
    }

    for (step_id, around, last_count, partial, limit, window, policy) in [
        (
            "near-expand-after-partial-page",
            json!({ "ref": prior_observation_ref }),
            6,
            true,
            json!({ "entries": 28, "tokens": 3400 }),
            json!({ "before_entries": 14, "after_entries": 8 }),
            "expand_window",
        ),
        (
            "near-shrink-after-single-hit",
            json!({ "time": "2026-05-06T10:06:00Z" }),
            1,
            false,
            json!({ "entries": 4, "tokens": 1000 }),
            json!({ "before_entries": 1, "after_entries": 1 }),
            "shrink_window",
        ),
        (
            "near-about-list-after-cross-incident-hint",
            json!({ "sequence": 10 }),
            2,
            false,
            json!({ "entries": 16, "tokens": 2600 }),
            json!({ "before_entries": 6, "after_entries": 4 }),
            "expand_scope",
        ),
    ] {
        let dimensions = if policy == "expand_scope" {
            json!({ "mode": "all", "scope": "abouts", "abouts": [about, sibling_about] })
        } else {
            json!({ "mode": "all", "scope": "current_about" })
        };
        push(
            items,
            item(
                run_id,
                "conformance.read.near",
                "read",
                about,
                step_id,
                "Adjust nearby navigation based on the previous result.",
                json!({
                    "current_ref": current_ref,
                    "known_refs": reader_refs,
                    "last_tool": "kernel_near",
                    "last_observed_refs": [prior_observation_ref, final_decision_ref],
                    "last_result_count": last_count,
                    "last_result_partial": partial,
                    "requested_move": requested_move("kernel_near", "around", around.clone()),
                    "requested_scope": dimensions.clone(),
                    "requested_bounds": requested_bounds(limit.clone(), window.clone()),
                    "remaining_budget": budget(3),
                    "operator_state": {
                        "decision": policy,
                        "why": "The next read should be bounded but adjusted to the observed result."
                    }
                }),
                read_tools.to_vec(),
                temporal_call("kernel_near", "around", around, dimensions, limit, window),
                json!({ "success": true, "observed_refs": [prior_observation_ref, final_decision_ref] }),
                json!({ "bounded": true, "contract_expected": true, "policy": policy }),
            ),
        );
    }

    push(
        items,
        item(
            run_id,
            "conformance.read.trace",
            "read",
            about,
            "trace-stop-after-final-page",
            "Stop instead of continuing trace when the last page is complete.",
            json!({
                "current_ref": current_ref,
                "known_refs": [prior_observation_ref, stale_decision_ref, final_decision_ref],
                "last_tool": "kernel_trace",
                "last_observed_refs": [prior_observation_ref, stale_decision_ref, final_decision_ref],
                "last_result_page": {
                    "entries": 12,
                    "has_more": false,
                    "next_cursor": null
                },
                "last_result_partial": false,
                "remaining_budget": budget(1),
                "requested_stop": requested_stop(
                    "evidence_or_unknown",
                    vec![
                        prior_observation_ref,
                        stale_decision_ref,
                        final_decision_ref,
                    ],
                    "sufficient_trace_complete"
                ),
                "operator_state": {
                    "decision": "stop_sufficient",
                    "evidence_sufficient": true
                }
            }),
            read_tools.to_vec(),
            stop_action(
                "evidence_or_unknown",
                vec![
                    prior_observation_ref,
                    stale_decision_ref,
                    final_decision_ref,
                ],
                "sufficient_trace_complete",
            ),
            json!({ "success": true, "continued_unnecessarily": false }),
            json!({ "bounded": true, "contract_expected": true, "policy": "stop_after_trace_complete" }),
        ),
    );

    for (step_id, arguments, relation_quality) in [
        (
            "write-memory-updates-state",
            write_memory_updates_state_arguments(),
            "rich",
        ),
        (
            "write-memory-contradicts-stale-hypothesis",
            write_memory_contradicts_arguments(),
            "rich",
        ),
        (
            "write-memory-contributes-to-derived-fact",
            write_memory_contributes_to_arguments(),
            "rich",
        ),
        (
            "write-memory-anemic-answer-link",
            write_memory_answers_arguments(),
            "anemic",
        ),
    ] {
        push(
            items,
            item(
                run_id,
                "conformance.write.smart",
                "write",
                about,
                step_id,
                "Write memory using only the strict smart-write contract.",
                json!({
                    "current_ref": format!("incident:mobile-login:draft:{step_id}"),
                    "known_refs": reader_refs,
                    "last_tool": "kernel_inspect",
                    "last_observed_refs": [prior_observation_ref, final_decision_ref],
                    "candidate_refs": reader_refs,
                    "read_context": {
                        "inspected_refs": [prior_observation_ref, prior_timeout_ref, policy_ref, constraint_ref],
                        "temporal_refs": [final_decision_ref, stale_decision_ref],
                        "ask_refs": [current_ref]
                    },
                    "draft_write": {
                        "intent": "record_memory",
                        "prepared_arguments": arguments.clone(),
                        "relation_requirement": "use only strict kernel_write_memory fields; do not add strategy or helper metadata"
                    },
                    "remaining_budget": budget(2),
                    "operator_state": {
                        "decision": "write_memory",
                        "relation_quality": relation_quality,
                        "forbidden_output_fields": ["strategy", "relation_strategy", "target_action"]
                    }
                }),
                full_tools.to_vec(),
                tool_call("kernel_write_memory", arguments),
                json!({
                    "success": true,
                    "dry_run": true,
                    "compiled_to": "kernel_ingest"
                }),
                json!({
                    "bounded": true,
                    "contract_expected": true,
                    "write_relation_quality": relation_quality,
                    "no_invented_helper_fields": true
                }),
            ),
        );
    }

    for (step_id, final_refs, reason, draft_relation) in [
        (
            "write-stop-before-invented-strategy-field",
            Vec::new(),
            "write_contract_has_no_strategy_field",
            json!({
                "ref": prior_observation_ref,
                "rel": "chosen_because",
                "class": "causal",
                "strategy": { "invented": true }
            }),
        ),
        (
            "write-stop-rich-relation-target-not-read",
            Vec::new(),
            "write_requires_target_read_before_rich_relation",
            json!({
                "ref": "incident:mobile-login:observation:not-visible",
                "rel": "supports",
                "class": "evidential"
            }),
        ),
    ] {
        push(
            items,
            item(
                run_id,
                "conformance.write.failfast",
                "write",
                about,
                step_id,
                "Fail fast instead of inventing smart-write fields or relation proof.",
                json!({
                    "current_ref": format!("incident:mobile-login:draft:{step_id}"),
                    "known_refs": reader_refs,
                    "last_tool": "kernel_near",
                    "last_observed_refs": [prior_observation_ref],
                    "candidate_refs": [prior_observation_ref],
                    "read_context": {
                        "inspected_refs": [prior_observation_ref]
                    },
                    "draft_write": {
                        "intent": "record_decision",
                        "proposed_relation": draft_relation
                    },
                    "remaining_budget": budget(1),
                    "operator_state": {
                        "decision": "stop_invalid_write_contract",
                        "why": reason
                    }
                }),
                full_tools.to_vec(),
                stop_action("evidence_or_unknown", final_refs, reason),
                json!({ "success": true, "write_attempted": false, "failfast": true }),
                json!({
                    "bounded": true,
                    "contract_expected": true,
                    "expected_failfast": true,
                    "negative_example": "write_contract"
                }),
            ),
        );
    }

    for (step_id, arguments, entries) in [
        (
            "ingest-canonical-memory-with-constraint",
            ingest_constraint_arguments(),
            2,
        ),
        (
            "ingest-canonical-memory-derived-values",
            ingest_derived_values_arguments(),
            3,
        ),
    ] {
        push(
            items,
            item(
                run_id,
                "conformance.write.ingest",
                "write",
                about,
                step_id,
                "Use canonical ingest when a complete typed memory payload is visible.",
                json!({
                    "current_ref": format!("incident:mobile-login:entry:{step_id}"),
                    "known_refs": reader_refs,
                    "last_tool": "kernel_write_memory",
                    "last_observed_refs": [prior_observation_ref, final_decision_ref],
                    "canonical_payload_ready": true,
                    "canonical_payload": arguments.clone(),
                    "remaining_budget": budget(1),
                    "operator_state": {
                        "decision": "canonical_ingest",
                        "payload_entries": entries
                    }
                }),
                full_tools.to_vec(),
                tool_call("kernel_ingest", arguments),
                json!({ "success": true, "dry_run": true, "entries": entries }),
                json!({ "bounded": true, "contract_expected": true, "canonical_write": true }),
            ),
        );
    }
}

fn push(items: &mut Vec<TrajectoryItem>, item: TrajectoryItem) {
    items.push(item);
}

fn writer_pre_read_candidate_details(candidates: Vec<(&str, &str, u64, &str)>) -> Vec<Value> {
    candidates
        .into_iter()
        .map(|(reference, role, priority, relation_hint)| {
            json!({
                "ref": reference,
                "role": role,
                "turn_kind": reference
                    .rsplit_once(':')
                    .map(|(_, kind)| kind)
                    .unwrap_or("unknown"),
                "relative_position": role
                    .split_once('_')
                    .map(|(position, _)| position)
                    .unwrap_or("unknown"),
                "priority": priority,
                "relation_hint": relation_hint,
            })
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn push_writer_pre_read_near(
    items: &mut Vec<TrajectoryItem>,
    run_id: &str,
    read_tools: &[String],
    about: &str,
    step_id: &str,
    entry_ref: &str,
    target_ref: &str,
    candidate_ref_details: Vec<Value>,
    limit: Value,
    window: Value,
    goal: &str,
    observed_refs: Vec<&str>,
) {
    let mut visible_state = writer_pre_read_visible_state(
        entry_ref,
        candidate_ref_details,
        Vec::new(),
        None,
        Vec::new(),
        None,
        None,
        3,
    );
    visible_state["requested_bounds"] = requested_bounds(limit.clone(), window.clone());
    push(
        items,
        item(
            run_id,
            "conformance.writer_pre_read",
            "write_context_read",
            about,
            step_id,
            goal,
            visible_state,
            read_tools.to_vec(),
            temporal_call_for_about(
                about,
                "kernel_near",
                "around",
                json!({ "ref": target_ref }),
                json!({ "mode": "all", "scope": "current_about" }),
                limit,
                window,
            ),
            json!({
                "success": true,
                "observed_refs": observed_refs,
                "partial_result": true,
                "page": {
                    "returned": 3,
                    "total": 9,
                    "has_more": true,
                    "next_cursor": target_ref
                }
            }),
            json!({
                "bounded": true,
                "contract_expected": true,
                "writer_pre_read_contract": true
            }),
        ),
    );
}

#[allow(clippy::too_many_arguments)]
fn push_writer_pre_read_inspect(
    items: &mut Vec<TrajectoryItem>,
    run_id: &str,
    read_tools: &[String],
    about: &str,
    step_id: &str,
    entry_ref: &str,
    target_ref: &str,
    candidate_ref_details: Vec<Value>,
    goal: &str,
) {
    push(
        items,
        item(
            run_id,
            "conformance.writer_pre_read",
            "write_context_read",
            about,
            step_id,
            goal,
            writer_pre_read_visible_state(
                entry_ref,
                candidate_ref_details,
                vec![target_ref.to_string()],
                Some("kernel_near"),
                vec![target_ref.to_string()],
                Some(json!({
                    "returned": 3,
                    "total": 9,
                    "has_more": true,
                    "next_cursor": target_ref
                })),
                Some(true),
                2,
            ),
            read_tools.to_vec(),
            tool_call("kernel_inspect", inspection_request(target_ref)),
            json!({
                "success": true,
                "observed_refs": [target_ref, entry_ref],
                "partial_result": false
            }),
            json!({
                "bounded": true,
                "contract_expected": true,
                "writer_pre_read_contract": true
            }),
        ),
    );
}

#[allow(clippy::too_many_arguments)]
fn push_writer_pre_read_trace(
    items: &mut Vec<TrajectoryItem>,
    run_id: &str,
    read_tools: &[String],
    about: &str,
    step_id: &str,
    entry_ref: &str,
    target_ref: &str,
    candidate_ref_details: Vec<Value>,
    goal: &str,
) {
    push(
        items,
        item(
            run_id,
            "conformance.writer_pre_read",
            "write_context_read",
            about,
            step_id,
            goal,
            writer_pre_read_visible_state(
                entry_ref,
                candidate_ref_details,
                vec![target_ref.to_string()],
                Some("kernel_inspect"),
                vec![target_ref.to_string(), entry_ref.to_string()],
                None,
                Some(false),
                1,
            ),
            read_tools.to_vec(),
            tool_call(
                "kernel_trace",
                json!({
                    "from": entry_ref,
                    "to": target_ref,
                    "goal": "Prove the writer pre-read relation target before writing memory.",
                    "role": "operator",
                    "budget": { "depth": 2, "tokens": 1600 },
                    "page": { "entries": 16 }
                }),
            ),
            json!({
                "success": true,
                "observed_refs": [entry_ref, target_ref],
                "partial_result": false,
                "page": {
                    "returned": 2,
                    "total": 2,
                    "has_more": false
                }
            }),
            json!({
                "bounded": true,
                "contract_expected": true,
                "writer_pre_read_contract": true
            }),
        ),
    );
}

#[allow(clippy::too_many_arguments)]
fn writer_pre_read_visible_state(
    entry_ref: &str,
    candidate_ref_details: Vec<Value>,
    known_refs: Vec<String>,
    last_tool: Option<&str>,
    last_observed_refs: Vec<String>,
    last_result_page: Option<Value>,
    last_result_partial: Option<bool>,
    remaining_tool_calls: usize,
) -> Value {
    let candidate_refs = candidate_ref_details
        .iter()
        .filter_map(|detail| detail.get("ref").and_then(Value::as_str))
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    json!({
        "current_ref": entry_ref,
        "candidate_refs": candidate_refs,
        "candidate_ref_details": candidate_ref_details,
        "known_refs": known_refs,
        "last_tool": last_tool,
        "last_observed_refs": last_observed_refs,
        "last_result_page": last_result_page,
        "last_result_partial": last_result_partial,
        "remaining_budget": {
            "tool_calls": remaining_tool_calls,
            "context_chars": DEFAULT_CONTEXT_CHARS
        },
        "operator_state": {
            "decision_boundary": "read_before_write",
            "why": "The writer must inspect enough prior memory to justify the next relation without inventing it."
        }
    })
}

fn with_candidate_pool(mut state: Value, candidate_pool: &str) -> Value {
    state["candidate_pool"] = json!(candidate_pool);
    state
}

#[allow(clippy::too_many_arguments)]
fn push_writer_pre_read_inspect_ambiguous(
    items: &mut Vec<TrajectoryItem>,
    run_id: &str,
    read_tools: &[String],
    about: &str,
    step_id: &str,
    entry_ref: &str,
    target_ref: &str,
    candidate_ref_details: Vec<Value>,
    last_observed_refs: Vec<&str>,
    goal: &str,
) {
    let last_observed_refs = last_observed_refs
        .into_iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    push(
        items,
        item(
            run_id,
            "conformance.writer_pre_read",
            "write_context_read",
            about,
            step_id,
            goal,
            with_candidate_pool(
                writer_pre_read_visible_state(
                    entry_ref,
                    candidate_ref_details,
                    vec![target_ref.to_string()],
                    Some("kernel_near"),
                    last_observed_refs,
                    Some(json!({
                        "returned": 3,
                        "total": 11,
                        "has_more": true,
                        "next_cursor": target_ref
                    })),
                    Some(true),
                    2,
                ),
                "ambiguous",
            ),
            read_tools.to_vec(),
            tool_call("kernel_inspect", inspection_request(target_ref)),
            json!({
                "success": true,
                "observed_refs": [target_ref, entry_ref],
                "partial_result": false
            }),
            json!({
                "bounded": true,
                "candidate_pool": "ambiguous",
                "contract_expected": true,
                "writer_pre_read_contract": true
            }),
        ),
    );
}

#[allow(clippy::too_many_arguments)]
fn push_writer_pre_read_trace_continue(
    items: &mut Vec<TrajectoryItem>,
    run_id: &str,
    read_tools: &[String],
    about: &str,
    step_id: &str,
    entry_ref: &str,
    target_ref: &str,
    candidate_ref_details: Vec<Value>,
    page_cursor: &str,
    last_observed_refs: Vec<&str>,
    goal: &str,
) {
    let last_observed_refs = last_observed_refs
        .into_iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    push(
        items,
        item(
            run_id,
            "conformance.writer_pre_read",
            "write_context_read",
            about,
            step_id,
            goal,
            writer_pre_read_visible_state(
                entry_ref,
                candidate_ref_details,
                vec![target_ref.to_string()],
                Some("kernel_trace"),
                last_observed_refs,
                Some(json!({
                    "returned": 16,
                    "total": 31,
                    "has_more": true,
                    "next_cursor": page_cursor
                })),
                Some(true),
                1,
            ),
            read_tools.to_vec(),
            tool_call(
                "kernel_trace",
                json!({
                    "from": entry_ref,
                    "to": target_ref,
                    "goal": "Continue the writer pre-read trace page before writing memory.",
                    "role": "operator",
                    "budget": { "depth": 2, "tokens": 1600 },
                    "page": { "entries": 16, "cursor": page_cursor }
                }),
            ),
            json!({
                "success": true,
                "observed_refs": [entry_ref, target_ref],
                "partial_result": false,
                "page": {
                    "returned": 12,
                    "total": 31,
                    "has_more": false
                }
            }),
            json!({
                "bounded": true,
                "contract_expected": true,
                "writer_pre_read_contract": true
            }),
        ),
    );
}

#[allow(clippy::too_many_arguments)]
fn push_writer_pre_read_stop(
    items: &mut Vec<TrajectoryItem>,
    run_id: &str,
    read_tools: &[String],
    about: &str,
    step_id: &str,
    entry_ref: &str,
    target_ref: &str,
    candidate_ref_details: Vec<Value>,
    last_observed_refs: Vec<&str>,
    final_refs: Vec<&str>,
    reason: &str,
) {
    let last_observed_refs = last_observed_refs
        .into_iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let known_refs = final_refs
        .iter()
        .map(|reference| (*reference).to_string())
        .collect::<Vec<_>>();
    push(
        items,
        item(
            run_id,
            "conformance.writer_pre_read",
            "write_context_read",
            about,
            step_id,
            reason,
            writer_pre_read_visible_state(
                entry_ref,
                candidate_ref_details,
                known_refs,
                Some("kernel_trace"),
                last_observed_refs,
                Some(json!({
                    "returned": 12,
                    "total": 31,
                    "has_more": false
                })),
                Some(false),
                0,
            ),
            read_tools.to_vec(),
            stop_action("evidence_or_unknown", final_refs, reason),
            json!({
                "success": true,
                "observed_refs": [entry_ref, target_ref],
                "partial_result": false
            }),
            json!({
                "bounded": true,
                "contract_expected": true,
                "policy": "stop_sufficient",
                "writer_pre_read_contract": true
            }),
        ),
    );
}

#[allow(clippy::too_many_arguments)]
fn push_writer_pre_read_stop_boundary(
    items: &mut Vec<TrajectoryItem>,
    run_id: &str,
    read_tools: &[String],
    about: &str,
    step_id: &str,
    goal: &str,
    entry_ref: &str,
    target_ref: &str,
    candidate_ref_details: Vec<Value>,
    last_observed_refs: Vec<&str>,
    final_refs: Vec<&str>,
) {
    let last_observed_refs = last_observed_refs
        .into_iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let known_refs = final_refs
        .iter()
        .map(|reference| (*reference).to_string())
        .collect::<Vec<_>>();
    push(
        items,
        item(
            run_id,
            "conformance.writer_pre_read",
            "write_context_read",
            about,
            step_id,
            goal,
            writer_pre_read_visible_state(
                entry_ref,
                candidate_ref_details,
                known_refs,
                Some("kernel_trace"),
                last_observed_refs,
                Some(json!({
                    "returned": 12,
                    "total": 12,
                    "has_more": false
                })),
                Some(false),
                0,
            ),
            read_tools.to_vec(),
            stop_action("evidence_or_unknown", final_refs, "sufficient_evidence"),
            json!({
                "success": true,
                "observed_refs": [entry_ref, target_ref],
                "partial_result": false,
                "page": {
                    "returned": 12,
                    "total": 12,
                    "has_more": false
                }
            }),
            json!({
                "bounded": true,
                "contract_expected": true,
                "hard_negative_family": "stop_vs_trace",
                "policy": "stop_sufficient",
                "writer_pre_read_contract": true
            }),
        ),
    );
}

// These helpers are fixture factories: keeping each KMP field explicit makes the
// generated conformance rows easier to audit than a dense positional builder.
#[allow(clippy::too_many_arguments)]
fn push_temporal_tool(
    items: &mut Vec<TrajectoryItem>,
    run_id: &str,
    tool: &str,
    cursor_key: &str,
    cursor: Value,
    step_id: &str,
    goal: &str,
    allowed_tools: Vec<String>,
) {
    let about = "incident:mobile-login";
    let current_ref = "incident:mobile-login:decision:refresh-retry";
    push(
        items,
        item(
            run_id,
            "conformance.read.temporal",
            "read",
            about,
            step_id,
            goal,
            json!({
                "current_ref": current_ref,
                "known_refs": [
                    "incident:mobile-login:observation:401-refresh-race",
                    current_ref
                ],
                "last_tool": "kernel_near",
                "last_observed_refs": [current_ref],
                "requested_move": requested_move(tool, cursor_key, cursor.clone()),
                "requested_scope": json!({ "mode": "all", "scope": "current_about" }),
                "requested_bounds": requested_bounds(
                    json!({ "entries": 12, "tokens": 2400 }),
                    json!({ "before_entries": 6, "after_entries": 0 })
                ),
                "remaining_budget": budget(3),
                "operator_state": {
                    "decision": tool,
                    "cursor_key": cursor_key
                }
            }),
            allowed_tools,
            temporal_call(
                tool,
                cursor_key,
                cursor,
                json!({ "mode": "all", "scope": "current_about" }),
                json!({ "entries": 12, "tokens": 2400 }),
                json!({ "before_entries": 6, "after_entries": 0 }),
            ),
            json!({
                "success": true,
                "observed_refs": ["incident:mobile-login:observation:401-refresh-race", current_ref]
            }),
            json!({ "bounded": true, "contract_expected": true }),
        ),
    );
}

#[allow(clippy::too_many_arguments)]
fn item(
    run_id: &str,
    task_family: &str,
    mode: &str,
    about: &str,
    step_id: &str,
    goal: &str,
    visible_state: Value,
    allowed_tools: Vec<String>,
    target_action: Value,
    observed_outcome: Value,
    quality: Value,
) -> TrajectoryItem {
    TrajectoryItem {
        schema_version: SCHEMA_VERSION,
        run_id: run_id.to_string(),
        task_family: task_family.to_string(),
        mode: mode.to_string(),
        source: "kernel_operator.conformance.synthetic".to_string(),
        about: about.to_string(),
        step_id: format!("{run_id}:{step_id}"),
        step_index: 0,
        goal: goal.to_string(),
        visible_state,
        allowed_tools,
        target_action,
        observed_outcome: Some(observed_outcome),
        quality,
    }
}

fn tool_call(tool: &str, arguments: Value) -> Value {
    json!({
        "type": "tool_call",
        "tool": tool,
        "arguments": arguments
    })
}

fn temporal_call(
    tool: &str,
    cursor_key: &str,
    cursor: Value,
    dimensions: Value,
    limit: Value,
    window: Value,
) -> Value {
    temporal_call_for_about(
        "incident:mobile-login",
        tool,
        cursor_key,
        cursor,
        dimensions,
        limit,
        window,
    )
}

fn temporal_call_for_about(
    about: &str,
    tool: &str,
    cursor_key: &str,
    cursor: Value,
    dimensions: Value,
    limit: Value,
    window: Value,
) -> Value {
    let mut arguments = serde_json::Map::new();
    arguments.insert("about".to_string(), json!(about));
    arguments.insert(cursor_key.to_string(), cursor);
    arguments.insert("dimensions".to_string(), dimensions);
    arguments.insert(
        "include".to_string(),
        json!({ "evidence": true, "raw_refs": false, "relations": true }),
    );
    arguments.insert("limit".to_string(), limit);
    arguments.insert("budget".to_string(), json!({ "depth": 3, "tokens": 2400 }));
    arguments.insert("window".to_string(), window);
    tool_call(tool, Value::Object(arguments))
}

fn requested_move(kind: &str, cursor_key: &str, cursor: Value) -> Value {
    json!({
        "kind": kind,
        "cursor_key": cursor_key,
        "cursor": cursor
    })
}

fn requested_wake(intent: &str, dimensions: Value, tokens: usize, depth: usize) -> Value {
    json!({
        "role": "operator",
        "intent": intent,
        "dimensions": dimensions,
        "budget": { "tokens": tokens, "depth": depth },
        "depth": depth
    })
}

fn requested_ask(
    question: &str,
    answer_policy: &str,
    dimensions: Value,
    tokens: usize,
    depth: usize,
) -> Value {
    json!({
        "question": question,
        "answer_policy": answer_policy,
        "dimensions": dimensions,
        "budget": { "tokens": tokens },
        "depth": depth
    })
}

fn requested_bounds(limit: Value, window: Value) -> Value {
    json!({
        "limit": limit,
        "window": window,
        "budget": { "depth": 3, "tokens": 2400 },
        "include": { "evidence": true, "raw_refs": false, "relations": true }
    })
}

fn requested_trace(from: &str, to: &str, goal: &str, page: Value) -> Value {
    json!({
        "from": from,
        "to": to,
        "goal": goal,
        "role": "operator",
        "budget": { "depth": 2, "tokens": 2400 },
        "page": page
    })
}

fn inspection_request(ref_id: &str) -> Value {
    json!({
        "ref": ref_id,
        "include": {
            "details": true,
            "incoming": true,
            "outgoing": true,
            "raw": false
        }
    })
}

fn requested_stop(answer_policy: &str, final_refs: Vec<&str>, reason: &str) -> Value {
    json!({
        "answer_policy": answer_policy,
        "final_refs": final_refs,
        "reason": reason
    })
}

fn stop_action(answer_policy: &str, final_refs: Vec<&str>, reason: &str) -> Value {
    json!({
        "type": "stop",
        "answer_policy": answer_policy,
        "final_refs": final_refs,
        "reason": reason
    })
}

fn budget(tool_calls: usize) -> Value {
    json!({
        "tool_calls": tool_calls,
        "context_chars": DEFAULT_CONTEXT_CHARS
    })
}

fn write_memory_arguments(rich: bool) -> Value {
    if rich {
        action_arguments_fixture(
            "write_memory/rich.json",
            include_str!("../../fixtures/conformance/write_memory/rich.json"),
        )
    } else {
        action_arguments_fixture(
            "write_memory/anemic.json",
            include_str!("../../fixtures/conformance/write_memory/anemic.json"),
        )
    }
}

fn write_memory_anemic_arguments() -> Value {
    write_memory_arguments(false)
}

fn write_memory_rich_without_delta_arguments() -> Value {
    action_arguments_fixture(
        "write_memory/rich_without_delta.json",
        include_str!("../../fixtures/conformance/write_memory/rich_without_delta.json"),
    )
}

fn write_memory_anemic_without_delta_arguments() -> Value {
    action_arguments_fixture(
        "write_memory/anemic_without_delta.json",
        include_str!("../../fixtures/conformance/write_memory/anemic_without_delta.json"),
    )
}

fn write_memory_updates_state_arguments() -> Value {
    action_arguments_fixture(
        "write_memory/updates_state.json",
        include_str!("../../fixtures/conformance/write_memory/updates_state.json"),
    )
}

fn write_memory_contradicts_arguments() -> Value {
    action_arguments_fixture(
        "write_memory/contradicts.json",
        include_str!("../../fixtures/conformance/write_memory/contradicts.json"),
    )
}

fn write_memory_contributes_to_arguments() -> Value {
    action_arguments_fixture(
        "write_memory/contributes_to.json",
        include_str!("../../fixtures/conformance/write_memory/contributes_to.json"),
    )
}

fn write_memory_answers_arguments() -> Value {
    action_arguments_fixture(
        "write_memory/answers.json",
        include_str!("../../fixtures/conformance/write_memory/answers.json"),
    )
}

fn ingest_arguments() -> Value {
    action_arguments_fixture(
        "ingest/single_entry.json",
        include_str!("../../fixtures/conformance/ingest/single_entry.json"),
    )
}

fn ingest_multi_entry_arguments() -> Value {
    action_arguments_fixture(
        "ingest/multi_entry.json",
        include_str!("../../fixtures/conformance/ingest/multi_entry.json"),
    )
}

fn ingest_constraint_arguments() -> Value {
    action_arguments_fixture(
        "ingest/constraint.json",
        include_str!("../../fixtures/conformance/ingest/constraint.json"),
    )
}

fn ingest_derived_values_arguments() -> Value {
    action_arguments_fixture(
        "ingest/derived_values.json",
        include_str!("../../fixtures/conformance/ingest/derived_values.json"),
    )
}

fn validate_trajectories(
    trajectories: &[TrajectoryItem],
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut seen = BTreeSet::<&str>::new();
    for (index, item) in trajectories.iter().enumerate() {
        if !seen.insert(item.step_id.as_str()) {
            return Err(format!("duplicate step_id `{}`", item.step_id).into());
        }
        let diagnostic = kernel_operator_action_contract_diagnostic(&item.target_action);
        if let Some(violation) = diagnostic.violation() {
            return Err(format!(
                "trajectory {} target_action violates Operator contract: {}",
                item.step_id,
                violation.message()
            )
            .into());
        }
        if item.step_index != 0 {
            return Err(format!(
                "trajectory {} has non-normalized initial step_index {}",
                item.step_id, item.step_index
            )
            .into());
        }
        let _ = index;
    }
    Ok(())
}

fn write_jsonl(
    path: &Path,
    trajectories: &[TrajectoryItem],
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    for (index, item) in trajectories.iter().enumerate() {
        let mut value = serde_json::to_value(item)?;
        value["step_index"] = json!(index);
        writer.write_all(serde_json::to_string(&value)?.as_bytes())?;
        writer.write_all(b"\n")?;
    }
    writer.flush()?;
    Ok(())
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), Box<dyn Error + Send + Sync>> {
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    writer.write_all(serde_json::to_string_pretty(value)?.as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}

fn summary(
    args: &Args,
    trajectories: &[TrajectoryItem],
) -> Result<ExportSummary, Box<dyn Error + Send + Sync>> {
    let mut modes = BTreeMap::<String, usize>::new();
    let mut task_families = BTreeMap::<String, usize>::new();
    let mut target_actions = BTreeMap::<String, usize>::new();
    let mut contract_validation_failures = 0usize;
    let mut contract_validation_failure_phases = BTreeMap::<String, usize>::new();

    for item in trajectories {
        *modes.entry(item.mode.clone()).or_default() += 1;
        *task_families.entry(item.task_family.clone()).or_default() += 1;
        match item.target_action.get("type").and_then(Value::as_str) {
            Some("tool_call") => {
                let tool = item
                    .target_action
                    .get("tool")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                *target_actions
                    .entry(format!("tool_call:{tool}"))
                    .or_default() += 1;
            }
            Some("stop") => {
                *target_actions.entry("stop".to_string()).or_default() += 1;
            }
            Some(other) => {
                *target_actions.entry(other.to_string()).or_default() += 1;
            }
            None => {
                *target_actions.entry("unknown".to_string()).or_default() += 1;
            }
        }
        let diagnostic = kernel_operator_action_contract_diagnostic(&item.target_action);
        if let Some(violation) = diagnostic.violation() {
            contract_validation_failures += 1;
            *contract_validation_failure_phases
                .entry(violation.phase().as_str().to_string())
                .or_default() += 1;
        }
    }

    Ok(ExportSummary {
        exporter: EXPORTER,
        schema_version: SCHEMA_VERSION,
        generated_at_unix_seconds: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
        suite: args.suite.clone(),
        run_id: args.run_id.clone(),
        output: args.output.display().to_string(),
        trajectories: trajectories.len(),
        modes,
        task_families,
        target_actions,
        contract_validation_failures,
        contract_validation_failure_phases,
        notes: vec![
            "Synthetic conformance trajectories are protocol tests, not benchmark result claims."
                .to_string(),
            "The fail-fast write case teaches the Operator to stop instead of inventing rich relation proof."
                .to_string(),
            "Every target_action is validated against kernel-operator-action-contract-v1 before export."
                .to_string(),
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_conformance_trajectories_are_contract_valid_and_unique() {
        let trajectories = conformance_trajectories(DEFAULT_RUN_ID);
        validate_trajectories(&trajectories).expect("valid conformance trajectories");

        let step_ids = trajectories
            .iter()
            .map(|trajectory| trajectory.step_id.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(step_ids.len(), trajectories.len());
        assert!(trajectories.iter().any(|trajectory| {
            trajectory.target_action.get("tool").and_then(Value::as_str)
                == Some("kernel_write_memory")
        }));
        assert!(trajectories.iter().any(|trajectory| {
            trajectory.target_action.get("tool").and_then(Value::as_str) == Some("kernel_ingest")
        }));
        assert!(trajectories.iter().any(|trajectory| {
            trajectory.mode == "write"
                && trajectory
                    .quality
                    .get("expected_failfast")
                    .and_then(Value::as_bool)
                    == Some(true)
                && trajectory.target_action.get("type").and_then(Value::as_str) == Some("stop")
        }));
    }

    #[test]
    fn generated_read_generalization_trajectories_are_contract_valid_and_unique() {
        let trajectories = read_generalization_trajectories("kmp-operator-generalization-test");
        validate_trajectories(&trajectories).expect("valid read generalization trajectories");

        let step_ids = trajectories
            .iter()
            .map(|trajectory| trajectory.step_id.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(step_ids.len(), trajectories.len());
        assert!(
            trajectories
                .iter()
                .all(|trajectory| trajectory.mode == "read")
        );
        assert!(trajectories.iter().any(|trajectory| {
            trajectory.target_action.get("tool").and_then(Value::as_str) == Some("kernel_trace")
        }));
        assert!(trajectories.iter().any(|trajectory| {
            trajectory.target_action.get("tool").and_then(Value::as_str) == Some("kernel_inspect")
        }));
        assert!(trajectories.iter().any(|trajectory| {
            trajectory.target_action.get("type").and_then(Value::as_str) == Some("stop")
        }));
    }

    #[test]
    fn generated_read_rare_expansion_trajectories_are_contract_valid_and_unique() {
        let trajectories = read_rare_expansion_trajectories("kmp-operator-rare-read-test");
        validate_trajectories(&trajectories).expect("valid rare read trajectories");

        let step_ids = trajectories
            .iter()
            .map(|trajectory| trajectory.step_id.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(step_ids.len(), trajectories.len());
        assert!(
            trajectories
                .iter()
                .all(|trajectory| trajectory.mode == "read")
        );
        assert!(
            trajectories
                .iter()
                .filter(|trajectory| {
                    trajectory.target_action.get("tool").and_then(Value::as_str)
                        == Some("kernel_wake")
                })
                .count()
                >= 8
        );
        assert!(
            trajectories
                .iter()
                .filter(|trajectory| {
                    trajectory
                        .target_action
                        .pointer("/arguments/dimensions/scope")
                        .and_then(Value::as_str)
                        == Some("all_abouts")
                })
                .count()
                >= 8
        );
        assert!(
            trajectories
                .iter()
                .filter(|trajectory| {
                    trajectory.target_action.get("tool").and_then(Value::as_str)
                        == Some("kernel_trace")
                        && trajectory
                            .target_action
                            .pointer("/arguments/page/cursor")
                            .is_some()
                })
                .count()
                >= 8
        );
    }

    #[test]
    fn generated_read_api_mcp_v1_trajectories_cover_read_surface() {
        let trajectories = read_api_mcp_v1_trajectories("kmp-operator-read-api-mcp-v1-test");
        validate_trajectories(&trajectories).expect("valid read api mcp trajectories");

        let step_ids = trajectories
            .iter()
            .map(|trajectory| trajectory.step_id.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(step_ids.len(), trajectories.len());
        assert!(
            trajectories
                .iter()
                .all(|trajectory| trajectory.mode == "read")
        );

        for tool in [
            "kernel_wake",
            "kernel_ask",
            "kernel_near",
            "kernel_goto",
            "kernel_rewind",
            "kernel_forward",
            "kernel_trace",
            "kernel_inspect",
        ] {
            let count = trajectories
                .iter()
                .filter(|trajectory| {
                    trajectory.target_action.get("tool").and_then(Value::as_str) == Some(tool)
                })
                .count();
            assert!(count >= 20, "{tool} count was {count}");
        }

        let stop_count = trajectories
            .iter()
            .filter(|trajectory| {
                trajectory.target_action.get("type").and_then(Value::as_str) == Some("stop")
            })
            .count();
        assert!(stop_count >= 20, "stop count was {stop_count}");
        assert!(trajectories.iter().any(|trajectory| {
            trajectory
                .target_action
                .pointer("/arguments/dimensions/scope")
                .and_then(Value::as_str)
                == Some("abouts")
        }));
        assert!(trajectories.iter().any(|trajectory| {
            trajectory
                .target_action
                .pointer("/arguments/page/cursor")
                .is_some()
        }));
    }

    #[test]
    fn generated_writer_pre_read_trajectories_are_contract_valid_and_unique() {
        let trajectories = writer_pre_read_trajectories("kmp-operator-writer-pre-read-test");
        validate_trajectories(&trajectories).expect("valid writer pre-read trajectories");

        let step_ids = trajectories
            .iter()
            .map(|trajectory| trajectory.step_id.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(step_ids.len(), trajectories.len());
        assert!(
            trajectories
                .iter()
                .all(|trajectory| trajectory.mode == "write_context_read")
        );
        let expected_tools = kernel_operator_allowed_writer_pre_read_tools();
        assert!(
            trajectories
                .iter()
                .all(|trajectory| trajectory.allowed_tools == expected_tools)
        );
        assert!(trajectories.iter().any(|trajectory| {
            trajectory.target_action.get("tool").and_then(Value::as_str) == Some("kernel_near")
                && trajectory
                    .target_action
                    .pointer("/arguments/window/before_entries")
                    .and_then(Value::as_u64)
                    == Some(3)
        }));
        assert!(trajectories.iter().any(|trajectory| {
            trajectory.target_action.get("tool").and_then(Value::as_str) == Some("kernel_near")
                && trajectory
                    .target_action
                    .pointer("/arguments/window/before_entries")
                    .and_then(Value::as_u64)
                    == Some(12)
        }));
        assert!(trajectories.iter().any(|trajectory| {
            trajectory.target_action.get("tool").and_then(Value::as_str) == Some("kernel_inspect")
        }));
        assert!(trajectories.iter().any(|trajectory| {
            trajectory.target_action.get("tool").and_then(Value::as_str) == Some("kernel_trace")
                && trajectory
                    .target_action
                    .pointer("/arguments/page/entries")
                    .and_then(Value::as_u64)
                    == Some(16)
        }));
    }

    #[test]
    fn generated_writer_pre_read_v2_trajectories_cover_stop_and_pagination() {
        let trajectories = writer_pre_read_v2_trajectories("kmp-operator-writer-pre-read-v2-test");
        validate_trajectories(&trajectories).expect("valid writer pre-read v2 trajectories");

        assert!(trajectories.len() > writer_pre_read_trajectories("baseline").len());
        assert!(
            trajectories
                .iter()
                .filter(|trajectory| {
                    trajectory.target_action.get("type").and_then(Value::as_str) == Some("stop")
                })
                .count()
                >= 2
        );
        assert!(
            trajectories
                .iter()
                .filter(|trajectory| {
                    trajectory.target_action.get("tool").and_then(Value::as_str)
                        == Some("kernel_trace")
                        && trajectory
                            .target_action
                            .pointer("/arguments/page/cursor")
                            .is_some()
                })
                .count()
                >= 2
        );
        assert!(
            trajectories
                .iter()
                .filter(|trajectory| {
                    trajectory
                        .visible_state
                        .get("candidate_pool")
                        .and_then(Value::as_str)
                        == Some("ambiguous")
                })
                .count()
                >= 2
        );
        assert!(
            trajectories
                .iter()
                .filter(|trajectory| {
                    trajectory
                        .visible_state
                        .get("last_tool")
                        .and_then(Value::as_str)
                        == Some("kernel_trace")
                })
                .count()
                >= 4
        );
    }

    #[test]
    fn generated_writer_pre_read_v3_trajectories_are_contract_valid_and_balanced() {
        let trajectories = writer_pre_read_v3_trajectories("kmp-operator-writer-pre-read-v3-test");
        validate_trajectories(&trajectories).expect("valid writer pre-read v3 trajectories");

        let step_ids = trajectories
            .iter()
            .map(|trajectory| trajectory.step_id.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(step_ids.len(), trajectories.len());
        assert!(
            trajectories
                .iter()
                .all(|trajectory| trajectory.mode == "write_context_read")
        );
        let expected_tools = kernel_operator_allowed_writer_pre_read_tools();
        assert!(
            trajectories
                .iter()
                .all(|trajectory| trajectory.allowed_tools == expected_tools)
        );

        for tool in ["kernel_near", "kernel_inspect", "kernel_trace"] {
            let count = trajectories
                .iter()
                .filter(|trajectory| {
                    trajectory.target_action.get("tool").and_then(Value::as_str) == Some(tool)
                })
                .count();
            assert!(count >= 50, "{tool} count was {count}");
        }
        let stop_count = trajectories
            .iter()
            .filter(|trajectory| {
                trajectory.target_action.get("type").and_then(Value::as_str) == Some("stop")
            })
            .count();
        assert!(stop_count >= 50, "stop count was {stop_count}");
    }

    #[test]
    fn generated_writer_pre_read_v4_extends_v3_with_stop_trace_hard_negatives() {
        let trajectories = writer_pre_read_v4_trajectories("kmp-operator-writer-pre-read-v4-test");
        validate_trajectories(&trajectories).expect("valid writer pre-read v4 trajectories");

        let step_ids = trajectories
            .iter()
            .map(|trajectory| trajectory.step_id.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(step_ids.len(), trajectories.len());
        assert!(
            trajectories
                .iter()
                .all(|trajectory| trajectory.mode == "write_context_read")
        );
        let expected_tools = kernel_operator_allowed_writer_pre_read_tools();
        assert!(
            trajectories
                .iter()
                .all(|trajectory| trajectory.allowed_tools == expected_tools)
        );

        let hard_negative_count = trajectories
            .iter()
            .filter(|trajectory| {
                trajectory
                    .quality
                    .get("hard_negative_family")
                    .and_then(Value::as_str)
                    == Some("stop_vs_trace")
            })
            .count();
        assert_eq!(hard_negative_count, 216);

        let stop_zero_budget_count = trajectories
            .iter()
            .filter(|trajectory| {
                trajectory
                    .quality
                    .get("synthetic_family")
                    .and_then(Value::as_str)
                    == Some("stop_after_complete_trace_zero_budget")
                    && trajectory
                        .visible_state
                        .pointer("/remaining_budget/tool_calls")
                        .and_then(Value::as_u64)
                        == Some(0)
                    && trajectory
                        .visible_state
                        .pointer("/last_result_page/has_more")
                        .and_then(Value::as_bool)
                        == Some(false)
                    && trajectory.target_action.get("type").and_then(Value::as_str) == Some("stop")
            })
            .count();
        assert_eq!(stop_zero_budget_count, 72);
    }

    #[test]
    fn generated_writer_exec_v1_trajectories_are_contract_valid_and_cohesive() {
        let trajectories = writer_exec_v1_trajectories("kmp-operator-writer-exec-v1-test");
        validate_trajectories(&trajectories).expect("valid writer exec v1 trajectories");

        let step_ids = trajectories
            .iter()
            .map(|trajectory| trajectory.step_id.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(step_ids.len(), trajectories.len());
        assert_eq!(trajectories.len(), 234);
        assert!(
            trajectories
                .iter()
                .all(|trajectory| trajectory.mode == "write")
        );
        assert!(trajectories.iter().all(|trajectory| {
            trajectory.allowed_tools == kernel_operator_allowed_write_tools()
        }));

        let write_memory_count = trajectories
            .iter()
            .filter(|trajectory| {
                trajectory.target_action.get("tool").and_then(Value::as_str)
                    == Some("kernel_write_memory")
            })
            .count();
        let ingest_count = trajectories
            .iter()
            .filter(|trajectory| {
                trajectory.target_action.get("tool").and_then(Value::as_str)
                    == Some("kernel_ingest")
            })
            .count();
        let stop_count = trajectories
            .iter()
            .filter(|trajectory| {
                trajectory.target_action.get("type").and_then(Value::as_str) == Some("stop")
            })
            .count();
        assert_eq!(write_memory_count, 72);
        assert_eq!(ingest_count, 36);
        assert_eq!(stop_count, 126);

        for family in [
            "write_rich_chosen_because",
            "write_rich_contradicts",
            "write_anemic_follows",
            "write_updates_state",
            "ingest_single",
            "ingest_multidimensional",
            "missing_prepared_payload",
            "missing_read_context_proof",
            "invalid_relation",
            "about_scope_mismatch",
            "incomplete_canonical_payload",
            "duplicate_idempotency",
            "strict_required",
        ] {
            let count = trajectories
                .iter()
                .filter(|trajectory| {
                    trajectory
                        .quality
                        .get("synthetic_family")
                        .and_then(Value::as_str)
                        == Some(family)
                })
                .count();
            assert_eq!(count, 18, "{family} count was {count}");
        }
    }

    #[test]
    fn generated_golden_v3_trajectories_are_contract_valid_and_unique() {
        let trajectories = golden_v3_trajectories("kmp-operator-golden-v3-test");
        validate_trajectories(&trajectories).expect("valid golden v3 trajectories");

        assert!(trajectories.iter().any(|trajectory| {
            trajectory
                .step_id
                .ends_with("wake-after-empty-near-current-ref-visible")
                && trajectory.target_action.get("tool").and_then(Value::as_str)
                    == Some("kernel_wake")
        }));
    }

    #[test]
    fn generated_golden_v4_trajectories_are_contract_valid_and_unique() {
        let trajectories = golden_v4_trajectories("kmp-operator-golden-v4-test");
        validate_trajectories(&trajectories).expect("valid golden v4 trajectories");

        assert!(trajectories.iter().any(|trajectory| {
            trajectory
                .step_id
                .ends_with("near-by-ref-except-discarded-and-scratch-training-contrast")
                && trajectory.target_action.get("tool").and_then(Value::as_str)
                    == Some("kernel_near")
        }));
    }
}
