use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::error::Error;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::Value;

use underpass_operator_evaluation_application::EvaluateContractProfileCoverageUseCase;
use underpass_operator_evaluation_domain::{
    ContractCoverageProfile, ContractCoverageRatio, ContractTrainingCoverageObservation,
};
use underpass_operator_evaluation_infra::{JsonContractCoverageObserver, JsonlValueReader};
use underpass_operator_shared_domain::operator_allowed_full_tools;

const REPORTER: &str = "kernel-operator-contract-coverage-v1";
const ACTION_CONTRACT: &str = "kernel-operator-action-contract-v1";

#[derive(Debug, Clone, PartialEq, Eq)]
struct Args {
    trajectories: Option<PathBuf>,
    output: Option<PathBuf>,
    profile: Profile,
    fail_under: Option<u64>,
}

type Profile = ContractCoverageProfile;

#[derive(Debug, Serialize)]
struct CoverageReport {
    reporter: &'static str,
    action_contract: &'static str,
    generated_at_unix_seconds: u64,
    profile: &'static str,
    trajectories: Option<String>,
    fail_under: Option<u64>,
    overall_contract_coverage: CoverageRatio,
    profile_contract_coverage: CoverageRatio,
    training_coverage: Option<TrainingCoverage>,
    required_capabilities: Vec<CapabilityRow>,
    unsupported_mcp_tools: Vec<String>,
    notes: Vec<String>,
}

#[derive(Debug, Serialize)]
struct TrainingCoverage {
    target_capability_coverage: CoverageRatio,
    rows_total: usize,
    rows_included: usize,
    rows_skipped_by_profile: usize,
    row_parse_failures: usize,
    row_parse_failure_examples: Vec<String>,
    target_tools: BTreeMap<String, usize>,
    target_cursor_modes: BTreeMap<String, usize>,
    target_dimension_modes: BTreeMap<String, usize>,
    target_dimension_scopes: BTreeMap<String, usize>,
    target_dimension_scope_ids: BTreeMap<String, usize>,
    target_trace_page_modes: BTreeMap<String, usize>,
    target_answer_policies: BTreeMap<String, usize>,
    target_budget_details: BTreeMap<String, usize>,
    target_temporal_raw_refs: BTreeMap<String, usize>,
    target_inspect_raw: BTreeMap<String, usize>,
    target_write_memory_options: BTreeMap<String, usize>,
    target_write_memory_dry_run: BTreeMap<String, usize>,
    target_write_memory_strict: BTreeMap<String, usize>,
    target_write_memory_idempotency_key: BTreeMap<String, usize>,
    target_write_memory_read_context: BTreeMap<String, usize>,
    target_write_memory_current_evidence: BTreeMap<String, usize>,
    target_write_memory_source_kind: BTreeMap<String, usize>,
    target_write_memory_relation_proof: BTreeMap<String, usize>,
    target_ingest_dry_run: BTreeMap<String, usize>,
    target_ingest_dimensions: BTreeMap<String, usize>,
    target_ingest_relations: BTreeMap<String, usize>,
    target_ingest_evidence: BTreeMap<String, usize>,
    target_ingest_provenance: BTreeMap<String, usize>,
    target_action_contract_failures: usize,
    target_action_contract_failure_phases: BTreeMap<String, usize>,
    target_action_contract_failure_examples: Vec<String>,
    missing_capabilities: Vec<String>,
}

#[derive(Debug, Serialize)]
struct CapabilityRow {
    id: String,
    group: &'static str,
    required_for_profile: bool,
    contract_supported: bool,
    training_observed: Option<bool>,
}

#[derive(Debug, Serialize)]
struct CoverageRatio {
    covered: usize,
    total: usize,
    percent: f64,
}

fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let args = parse_args(env::args().skip(1))?;
    let trajectories = args.trajectories.as_deref().map(read_jsonl).transpose()?;
    let report = build_report(&args, trajectories.as_deref())?;
    let rendered = serde_json::to_string_pretty(&report)?;
    if let Some(output) = args.output.as_deref() {
        let file = File::create(output)?;
        let mut writer = BufWriter::new(file);
        writer.write_all(rendered.as_bytes())?;
        writer.write_all(b"\n")?;
        writer.flush()?;
    }
    println!("{rendered}");
    if let Some(fail_under) = args.fail_under {
        if report.profile_contract_coverage.percent < fail_under as f64 {
            return Err(format!(
                "profile contract coverage {:.2}% is below fail-under {}%",
                report.profile_contract_coverage.percent, fail_under
            )
            .into());
        }
        if let Some(training) = &report.training_coverage
            && training.target_capability_coverage.percent < fail_under as f64
        {
            return Err(format!(
                "target capability coverage {:.2}% is below fail-under {}%",
                training.target_capability_coverage.percent, fail_under
            )
            .into());
        }
    }
    if let Some(training) = &report.training_coverage
        && training.target_action_contract_failures > 0
    {
        return Err(format!(
            "target action contract validation failed for {} row(s)",
            training.target_action_contract_failures
        )
        .into());
    }
    if let Some(training) = &report.training_coverage
        && training.row_parse_failures > 0
    {
        return Err(format!(
            "coverage could not parse {} row(s)",
            training.row_parse_failures
        )
        .into());
    }
    Ok(())
}

fn build_report(
    args: &Args,
    trajectories: Option<&[Value]>,
) -> Result<CoverageReport, Box<dyn Error + Send + Sync>> {
    let mcp_tools = operator_allowed_full_tools()
        .into_iter()
        .collect::<BTreeSet<_>>();
    let observed = trajectories.map(|rows| observed_capabilities(rows, args.profile));
    let observed_capability_ids = observed
        .as_ref()
        .map(ContractTrainingCoverageObservation::capability_ids);
    let contract_report = EvaluateContractProfileCoverageUseCase::new().execute(
        args.profile,
        mcp_tools,
        observed_capability_ids,
    );
    let required_ids = args.profile.required_capabilities();
    let required_rows = contract_report
        .required_capabilities()
        .iter()
        .map(|row| CapabilityRow {
            id: row.capability().id().to_string(),
            group: row.capability().group(),
            required_for_profile: true,
            contract_supported: row.contract_supported(),
            training_observed: row.training_observed(),
        })
        .collect::<Vec<_>>();
    let training_coverage = observed.map(|observed| {
        let covered = required_ids
            .iter()
            .filter(|capability| observed.capabilities.contains(capability.id()))
            .count();
        let missing_capabilities = required_ids
            .iter()
            .filter(|capability| !observed.capabilities.contains(capability.id()))
            .map(|capability| capability.id().to_string())
            .collect();
        TrainingCoverage {
            target_capability_coverage: ratio(covered, required_ids.len()),
            rows_total: observed.rows_total,
            rows_included: observed.rows_included,
            rows_skipped_by_profile: observed.rows_skipped_by_profile,
            row_parse_failures: observed.row_parse_failures,
            row_parse_failure_examples: observed.row_parse_failure_examples,
            target_tools: observed.target_tools,
            target_cursor_modes: observed.cursor_modes,
            target_dimension_modes: observed.dimension_modes,
            target_dimension_scopes: observed.dimension_scopes,
            target_dimension_scope_ids: observed.dimension_scope_ids,
            target_trace_page_modes: observed.trace_page_modes,
            target_answer_policies: observed.answer_policies,
            target_budget_details: observed.budget_details,
            target_temporal_raw_refs: observed.temporal_raw_refs,
            target_inspect_raw: observed.inspect_raw,
            target_write_memory_options: observed.write_memory_options,
            target_write_memory_dry_run: observed.write_memory_dry_run,
            target_write_memory_strict: observed.write_memory_strict,
            target_write_memory_idempotency_key: observed.write_memory_idempotency_key,
            target_write_memory_read_context: observed.write_memory_read_context,
            target_write_memory_current_evidence: observed.write_memory_current_evidence,
            target_write_memory_source_kind: observed.write_memory_source_kind,
            target_write_memory_relation_proof: observed.write_memory_relation_proof,
            target_ingest_dry_run: observed.ingest_dry_run,
            target_ingest_dimensions: observed.ingest_dimensions,
            target_ingest_relations: observed.ingest_relations,
            target_ingest_evidence: observed.ingest_evidence,
            target_ingest_provenance: observed.ingest_provenance,
            target_action_contract_failures: observed.action_contract_failures,
            target_action_contract_failure_phases: observed.action_contract_failure_phases,
            target_action_contract_failure_examples: observed.action_contract_failure_examples,
            missing_capabilities,
        }
    });
    Ok(CoverageReport {
        reporter: REPORTER,
        action_contract: ACTION_CONTRACT,
        generated_at_unix_seconds: now_unix_seconds()?,
        profile: args.profile.name(),
        trajectories: args
            .trajectories
            .as_ref()
            .map(|path| path.display().to_string()),
        fail_under: args.fail_under,
        overall_contract_coverage: ratio_from_domain(
            contract_report.overall_contract_coverage(),
        ),
        profile_contract_coverage: ratio_from_domain(
            contract_report.profile_contract_coverage(),
        ),
        training_coverage,
        required_capabilities: required_rows,
        unsupported_mcp_tools: contract_report.unsupported_mcp_tools().to_vec(),
        notes: vec![
            "overall_contract_coverage compares Operator tools against the entire MCP tool list; write tools can be intentionally outside a read profile.".to_string(),
            "profile_contract_coverage must be 100% before placing Operator in front of that profile.".to_string(),
            "target_capability_coverage must be 100% for the training/eval set before claiming the model has learned that profile.".to_string(),
        ],
    })
}

fn observed_capabilities(rows: &[Value], profile: Profile) -> ContractTrainingCoverageObservation {
    JsonContractCoverageObserver::observe(rows, profile)
}

fn ratio(covered: usize, total: usize) -> CoverageRatio {
    let percent = if total == 0 {
        100.0
    } else {
        covered as f64 * 100.0 / total as f64
    };
    CoverageRatio {
        covered,
        total,
        percent,
    }
}

fn ratio_from_domain(ratio: ContractCoverageRatio) -> CoverageRatio {
    CoverageRatio {
        covered: ratio.covered(),
        total: ratio.total(),
        percent: ratio.percent(),
    }
}

fn parse_args(
    mut args: impl Iterator<Item = String>,
) -> Result<Args, Box<dyn Error + Send + Sync>> {
    let mut trajectories = None;
    let mut output = None;
    let mut profile = Profile::Read;
    let mut fail_under = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--trajectories" => {
                trajectories = Some(PathBuf::from(next_arg(&mut args, "--trajectories")?));
            }
            "--output" => output = Some(PathBuf::from(next_arg(&mut args, "--output")?)),
            "--profile" => profile = parse_profile(&next_arg(&mut args, "--profile")?)?,
            "--fail-under" => fail_under = Some(next_arg(&mut args, "--fail-under")?.parse()?),
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
        trajectories,
        output,
        profile,
        fail_under,
    })
}

fn parse_profile(value: &str) -> Result<Profile, Box<dyn Error + Send + Sync>> {
    ContractCoverageProfile::parse(value).map_err(|_| {
        format!("unknown profile `{value}`; expected read|full|writer-pre-read|write").into()
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
    "usage: underpass_operator_contract_coverage [--trajectories trajectories.jsonl] [--profile read|full|writer-pre-read|write] [--fail-under pct] [--output summary.json]".to_string()
}

fn read_jsonl(path: &Path) -> Result<Vec<Value>, Box<dyn Error + Send + Sync>> {
    JsonlValueReader::read(path).map_err(|error| error.into())
}

fn now_unix_seconds() -> Result<u64, Box<dyn Error + Send + Sync>> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_facing_prepared_write_counts_resolved_write_capabilities() {
        let user_payload = serde_json::json!({
            "about": "about:1",
            "allowed_tools": ["kernel_write_memory"],
            "mode": "write",
            "visible_state": {
                "draft_write": {
                    "prepared_arguments": {
                        "about": "about:1",
                        "intent": "record_decision",
                        "actor": "agent:writer",
                        "observed_at": "2026-05-17T10:00:00Z",
                        "scope": {"process": "about:1"},
                        "connect_to": [{
                            "ref": "node:1",
                            "rel": "updates_state",
                            "class": "causal",
                            "why": "The note updates the prior decision.",
                            "evidence": "The prior decision is visible in inspected refs."
                        }],
                        "current": {
                            "ref": "node:2",
                            "kind": "decision",
                            "summary": "Record the updated decision.",
                            "evidence": "The inspected node contains the prior decision."
                        },
                        "read_context": {
                            "inspected_refs": ["node:1"]
                        },
                        "idempotency_key": "about:1:write:1",
                        "options": {"dry_run": true, "strict": true}
                    }
                }
            }
        });
        let assistant_payload = serde_json::json!({
            "action": {
                "type": "prepared_tool_call",
                "tool": "kernel_write_memory",
                "source": "draft_write.prepared_arguments"
            }
        });
        let row = serde_json::json!({
            "messages": [
                {"role": "system", "content": "system"},
                {
                    "role": "user",
                    "content": serde_json::to_string(&user_payload)
                        .expect("user payload must serialize")
                },
                {
                    "role": "assistant",
                    "content": serde_json::to_string(&assistant_payload)
                        .expect("assistant payload must serialize")
                }
            ]
        });

        let coverage = observed_capabilities(&[row], Profile::Write);

        assert!(coverage.capabilities.contains("mode:write"));
        assert!(
            coverage
                .capabilities
                .contains("prepared.source:draft_write.prepared_arguments")
        );
        assert!(coverage.capabilities.contains("tool:kernel_write_memory"));
        assert!(coverage.capabilities.contains("write:relation_quality"));
        assert!(coverage.capabilities.contains("write:read_context_proof"));
        assert_eq!(coverage.target_tools.get("kernel_write_memory"), Some(&1));
    }

    #[test]
    fn writer_pre_read_profile_uses_writer_pre_read_tool_subset() {
        let args = Args {
            trajectories: None,
            output: None,
            profile: Profile::WriterPreRead,
            fail_under: None,
        };

        let report = build_report(&args, None).expect("writer pre-read report must build");

        assert_eq!(report.profile_contract_coverage.percent, 100.0);
        assert!(
            report
                .unsupported_mcp_tools
                .contains(&"kernel_wake".to_string())
        );
        assert!(
            report
                .unsupported_mcp_tools
                .contains(&"kernel_write_memory".to_string())
        );
        assert!(
            !report
                .unsupported_mcp_tools
                .contains(&"kernel_near".to_string())
        );
    }

    #[test]
    fn mixed_dataset_is_filtered_by_profile_before_counting_coverage() {
        let writer_pre_read_row = serde_json::json!({
            "mode": "write_context_read",
            "allowed_tools": ["kernel_trace"],
            "visible_state": {
                "last_tool": null,
                "candidate_pool": "ambiguous",
                "candidate_ref_details": [
                    {"role": "previous_subtask_answer"},
                    {"role": "same_subtask_question"}
                ]
            },
            "target_action": {
                "type": "tool_call",
                "tool": "kernel_trace",
                "arguments": {
                    "from": "node:1",
                    "to": "node:2",
                    "page": {"entries": 16, "cursor": "16"},
                    "budget": {"tokens": 2400, "depth": 2, "detail": "compact"}
                }
            }
        });
        let write_row = serde_json::json!({
            "about": "about:1",
            "mode": "write",
            "allowed_tools": ["kernel_write_memory"],
            "visible_state": {
                "draft_write": {
                    "prepared_arguments": {
                        "about": "about:1",
                        "intent": "record_decision",
                        "actor": "agent:writer",
                        "observed_at": "2026-05-17T10:00:00Z",
                        "scope": {"process": "about:1"},
                        "connect_to": [{
                            "ref": "node:1",
                            "rel": "depends_on",
                            "class": "causal",
                            "why": "The new node depends on prior evidence.",
                            "evidence": "The prior node is in inspected refs."
                        }],
                        "current": {
                            "ref": "node:2",
                            "kind": "decision",
                            "summary": "Record the next memory node.",
                            "evidence": "The inspected node contains the prior evidence."
                        },
                        "read_context": {
                            "inspected_refs": ["node:1"]
                        },
                        "idempotency_key": "about:1:write:2",
                        "options": {"dry_run": true, "strict": true}
                    }
                }
            },
            "target_action": {
                "type": "prepared_tool_call",
                "tool": "kernel_write_memory",
                "source": "draft_write.prepared_arguments"
            }
        });
        let rows = vec![writer_pre_read_row, write_row];

        let writer_pre_read = observed_capabilities(&rows, Profile::WriterPreRead);
        assert_eq!(writer_pre_read.rows_total, 2);
        assert_eq!(writer_pre_read.rows_included, 1);
        assert_eq!(writer_pre_read.rows_skipped_by_profile, 1);
        assert_eq!(writer_pre_read.target_tools.get("kernel_trace"), Some(&1));
        assert!(
            !writer_pre_read
                .target_tools
                .contains_key("kernel_write_memory")
        );
        assert!(
            !writer_pre_read
                .capabilities
                .contains("prepared.source:draft_write.prepared_arguments")
        );

        let write = observed_capabilities(&rows, Profile::Write);
        assert_eq!(write.rows_total, 2);
        assert_eq!(write.rows_included, 1);
        assert_eq!(write.rows_skipped_by_profile, 1);
        assert_eq!(write.target_tools.get("kernel_write_memory"), Some(&1));
        assert!(!write.target_tools.contains_key("kernel_trace"));
        assert!(!write.capabilities.contains("trace.page:continue"));
    }

    #[test]
    fn malformed_model_facing_rows_are_parse_failures_not_profile_skips() {
        let rows = vec![serde_json::json!({
            "id": "bad-sft-row",
            "messages": []
        })];

        let coverage = observed_capabilities(&rows, Profile::Read);

        assert_eq!(coverage.rows_total, 1);
        assert_eq!(coverage.rows_included, 0);
        assert_eq!(coverage.rows_skipped_by_profile, 0);
        assert_eq!(coverage.row_parse_failures, 1);
        assert_eq!(
            coverage.row_parse_failure_examples,
            vec!["bad-sft-row: unable to parse model-facing messages/action"]
        );
        assert!(coverage.capabilities.is_empty());
    }

    #[test]
    fn missing_mode_rows_are_parse_failures_not_read_profile_rows() {
        let rows = vec![serde_json::json!({
            "step_id": "missing-mode-row",
            "target_action": {
                "type": "tool_call",
                "tool": "kernel_inspect",
                "arguments": {
                    "ref": "node:1",
                    "include": {"details": true, "incoming": true, "outgoing": true, "raw": false}
                }
            }
        })];

        let coverage = observed_capabilities(&rows, Profile::Read);

        assert_eq!(coverage.rows_total, 1);
        assert_eq!(coverage.rows_included, 0);
        assert_eq!(coverage.rows_skipped_by_profile, 0);
        assert_eq!(coverage.row_parse_failures, 1);
        assert_eq!(
            coverage.row_parse_failure_examples,
            vec!["missing-mode-row: missing or unsupported operator mode"]
        );
        assert!(coverage.capabilities.is_empty());
    }

    #[test]
    fn unresolved_prepared_payload_is_not_counted_as_coverage() {
        let rows = vec![serde_json::json!({
            "step_id": "bad-prepared-write",
            "about": "about:1",
            "mode": "write",
            "allowed_tools": ["kernel_write_memory"],
            "visible_state": {
                "draft_write": {}
            },
            "target_action": {
                "type": "prepared_tool_call",
                "tool": "kernel_write_memory",
                "source": "draft_write.prepared_arguments"
            }
        })];

        let coverage = observed_capabilities(&rows, Profile::Write);

        assert_eq!(coverage.rows_total, 1);
        assert_eq!(coverage.rows_included, 1);
        assert_eq!(coverage.action_contract_failures, 1);
        assert_eq!(
            coverage.action_contract_failure_examples,
            vec![
                "bad-prepared-write: prepared_tool_call could not be resolved to executable tool_call"
            ]
        );
        assert!(coverage.capabilities.is_empty());
        assert!(!coverage.target_tools.contains_key("kernel_write_memory"));
    }

    #[test]
    fn model_facing_disallowed_tool_is_not_counted_as_coverage() {
        let user_payload = serde_json::json!({
            "id": "disallowed-tool-row",
            "about": "about:1",
            "allowed_tools": ["kernel_inspect"],
            "mode": "read",
            "visible_state": {}
        });
        let assistant_payload = serde_json::json!({
            "action": {
                "type": "tool_call",
                "tool": "kernel_near",
                "arguments": {
                    "about": "about:1",
                    "around": {"ref": "node:1"},
                    "dimensions": {"mode": "all", "scope": "current_about"},
                    "include": {"evidence": true, "raw_refs": false, "relations": true},
                    "limit": {"entries": 12, "tokens": 2400},
                    "budget": {"depth": 3, "tokens": 2400},
                    "window": {"before_entries": 6, "after_entries": 0}
                }
            }
        });
        let row = serde_json::json!({
            "id": "disallowed-tool-row",
            "messages": [
                {"role": "system", "content": "system"},
                {
                    "role": "user",
                    "content": serde_json::to_string(&user_payload)
                        .expect("user payload must serialize")
                },
                {
                    "role": "assistant",
                    "content": serde_json::to_string(&assistant_payload)
                        .expect("assistant payload must serialize")
                }
            ]
        });

        let coverage = observed_capabilities(&[row], Profile::Read);

        assert_eq!(coverage.rows_total, 1);
        assert_eq!(coverage.rows_included, 1);
        assert_eq!(coverage.action_contract_failures, 1);
        assert_eq!(
            coverage.action_contract_failure_examples,
            vec!["disallowed-tool-row: tool `kernel_near` is not allowed by row allowed_tools"]
        );
        assert!(coverage.capabilities.is_empty());
        assert!(!coverage.target_tools.contains_key("kernel_near"));
    }

    #[test]
    fn raw_target_rows_without_allowed_tools_are_parse_failures() {
        let rows = vec![serde_json::json!({
            "step_id": "missing-allowed-tools-row",
            "mode": "read",
            "target_action": {
                "type": "stop",
                "reason": "sufficient_evidence"
            }
        })];

        let coverage = observed_capabilities(&rows, Profile::Read);

        assert_eq!(coverage.rows_total, 1);
        assert_eq!(coverage.rows_included, 0);
        assert_eq!(coverage.rows_skipped_by_profile, 0);
        assert_eq!(coverage.row_parse_failures, 1);
        assert_eq!(
            coverage.row_parse_failure_examples,
            vec!["missing-allowed-tools-row: missing or invalid allowed_tools"]
        );
        assert!(coverage.capabilities.is_empty());
    }

    #[test]
    fn allowed_tools_outside_mode_are_parse_failures() {
        let rows = vec![serde_json::json!({
            "step_id": "read-row-with-write-tool",
            "mode": "read",
            "allowed_tools": ["kernel_near", "kernel_write_memory"],
            "target_action": {
                "type": "stop",
                "answer_policy": "evidence_or_unknown",
                "final_refs": [],
                "reason": "sufficient_evidence"
            }
        })];

        let coverage = observed_capabilities(&rows, Profile::Read);

        assert_eq!(coverage.rows_total, 1);
        assert_eq!(coverage.rows_included, 0);
        assert_eq!(coverage.row_parse_failures, 1);
        assert_eq!(
            coverage.row_parse_failure_examples,
            vec![
                "read-row-with-write-tool: allowed_tools outside mode `read`: kernel_write_memory"
            ]
        );
        assert!(coverage.capabilities.is_empty());
    }

    #[test]
    fn read_distribution_counters_expose_scope_ids_budget_and_raw_flags() {
        let rows = vec![
            serde_json::json!({
                "step_id": "valid-near",
                "mode": "read",
                "allowed_tools": ["kernel_near"],
                "target_action": {
                    "type": "tool_call",
                    "tool": "kernel_near",
                    "arguments": {
                        "about": "about:1",
                        "around": {"ref": "node:1"},
                        "dimensions": {
                            "mode": "all",
                            "scope": "current_about",
                            "scope_ids": ["about:1:dimension:agent"]
                        },
                        "include": {"evidence": true, "raw_refs": false, "relations": true},
                        "limit": {"entries": 12, "tokens": 2400},
                        "budget": {"depth": 3, "tokens": 2400, "detail": "full"},
                        "window": {"before_entries": 6, "after_entries": 0}
                    }
                }
            }),
            serde_json::json!({
                "step_id": "valid-inspect",
                "mode": "read",
                "allowed_tools": ["kernel_inspect"],
                "target_action": {
                    "type": "tool_call",
                    "tool": "kernel_inspect",
                    "arguments": {
                        "ref": "node:1",
                        "include": {"details": true, "incoming": true, "outgoing": true, "raw": false}
                    }
                }
            }),
            serde_json::json!({
                "step_id": "invalid-raw-refs",
                "mode": "read",
                "allowed_tools": ["kernel_near"],
                "target_action": {
                    "type": "tool_call",
                    "tool": "kernel_near",
                    "arguments": {
                        "about": "about:1",
                        "around": {"ref": "node:1"},
                        "dimensions": {"mode": "all", "scope": "current_about"},
                        "include": {"evidence": true, "raw_refs": true, "relations": true},
                        "limit": {"entries": 12, "tokens": 2400},
                        "budget": {"depth": 3, "tokens": 2400, "detail": "summary"},
                        "window": {"before_entries": 6, "after_entries": 0}
                    }
                }
            }),
        ];

        let coverage = observed_capabilities(&rows, Profile::Read);

        assert_eq!(coverage.dimension_scope_ids.get("present"), Some(&1));
        assert_eq!(coverage.budget_details.get("full"), Some(&1));
        assert_eq!(coverage.temporal_raw_refs.get("false"), Some(&1));
        assert!(!coverage.temporal_raw_refs.contains_key("true"));
        assert_eq!(coverage.inspect_raw.get("false"), Some(&1));
        assert_eq!(coverage.action_contract_failures, 1);
        assert_eq!(
            coverage
                .action_contract_failure_phases
                .get("tool_arguments"),
            Some(&1)
        );
        assert_eq!(
            coverage.action_contract_failure_examples,
            vec!["invalid-raw-refs: action.arguments.include.raw_refs must be false"]
        );
    }

    #[test]
    fn write_distribution_counters_expose_safe_profile_and_ingest_shape() {
        let rows = vec![
            serde_json::json!({
                "mode": "write",
                "allowed_tools": ["kernel_write_memory"],
                "target_action": {
                    "type": "tool_call",
                    "tool": "kernel_write_memory",
                    "arguments": {
                        "about": "incident:1",
                        "intent": "record_decision",
                        "actor": "agent:writer",
                        "observed_at": "2026-05-17T10:00:00Z",
                        "source_kind": "agent",
                        "scope": {"process": "incident:1"},
                        "current": {
                            "kind": "decision",
                            "summary": "Use the cached answer.",
                            "evidence": "The inspected node already contains the final answer."
                        },
                        "connect_to": [{
                            "ref": "incident:1:observation:cached-answer",
                            "rel": "chosen_because",
                            "class": "causal",
                            "why": "The cached answer matches the requested evidence.",
                            "evidence": "The inspected node already contains the final answer.",
                            "confidence": "high"
                        }],
                        "read_context": {"inspected_refs": ["incident:1:observation:cached-answer"]},
                        "idempotency_key": "incident:1:write:1",
                        "options": {"dry_run": true, "strict": true}
                    }
                }
            }),
            serde_json::json!({
                "mode": "write",
                "allowed_tools": ["kernel_ingest"],
                "target_action": {
                    "type": "tool_call",
                    "tool": "kernel_ingest",
                    "arguments": {
                        "about": "incident:1",
                        "idempotency_key": "incident:1:ingest:append",
                        "dry_run": false,
                        "memory": {
                            "dimensions": [],
                            "entries": [{
                                "id": "incident:1:entry:append",
                                "kind": "observation",
                                "text": "The append reuses existing dimensions.",
                                "coordinates": [{
                                    "dimension": "agent",
                                    "scope_id": "agent:solver",
                                    "sequence": 2
                                }]
                            }]
                        }
                    }
                }
            }),
        ];

        let coverage = observed_capabilities(&rows, Profile::Write);

        assert_eq!(coverage.write_memory_options.get("present"), Some(&1));
        assert_eq!(coverage.write_memory_dry_run.get("true"), Some(&1));
        assert_eq!(coverage.write_memory_strict.get("true"), Some(&1));
        assert_eq!(
            coverage.write_memory_idempotency_key.get("present"),
            Some(&1)
        );
        assert_eq!(coverage.write_memory_read_context.get("present"), Some(&1));
        assert_eq!(
            coverage.write_memory_current_evidence.get("present"),
            Some(&1)
        );
        assert_eq!(coverage.write_memory_source_kind.get("present"), Some(&1));
        assert_eq!(
            coverage
                .write_memory_relation_proof
                .get("non_structural_complete"),
            Some(&1)
        );
        assert_eq!(coverage.ingest_dry_run.get("false"), Some(&1));
        assert_eq!(coverage.ingest_dimensions.get("empty"), Some(&1));
        assert_eq!(coverage.ingest_relations.get("absent"), Some(&1));
        assert_eq!(coverage.ingest_evidence.get("absent"), Some(&1));
        assert_eq!(coverage.ingest_provenance.get("absent"), Some(&1));
    }

    #[test]
    fn write_distribution_counters_expose_structural_relation_without_proof() {
        let rows = vec![serde_json::json!({
            "mode": "write",
            "allowed_tools": ["kernel_write_memory"],
            "target_action": {
                "type": "tool_call",
                "tool": "kernel_write_memory",
                "arguments": {
                    "about": "incident:1",
                    "intent": "record_decision",
                    "actor": "agent:writer",
                    "observed_at": "2026-05-17T10:00:00Z",
                    "scope": {"process": "incident:1"},
                    "current": {
                        "kind": "decision",
                        "summary": "Attach the node structurally.",
                        "evidence": "The node belongs to the current incident scope."
                    },
                    "connect_to": [{
                        "ref": "incident:1:scope:main",
                        "rel": "scoped_to",
                        "class": "structural"
                    }],
                    "read_context": {},
                    "idempotency_key": "incident:1:write:structural",
                    "options": {"dry_run": true, "strict": true}
                }
            }
        })];

        let coverage = observed_capabilities(&rows, Profile::Write);

        assert_eq!(
            coverage
                .write_memory_relation_proof
                .get("structural_without_proof"),
            Some(&1)
        );
    }
}
