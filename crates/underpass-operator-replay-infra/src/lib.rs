use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::error::Error;
use std::fmt;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;

use serde::Serialize;
use serde_json::Value;
use underpass_operator_replay_domain::{ReplayPrediction, ReplayRunReport, ReplayTrajectory};
use underpass_operator_shared_domain::operator_allowed_tools_for_mode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayInfraError {
    message: String,
}

impl ReplayInfraError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for ReplayInfraError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for ReplayInfraError {}

pub struct JsonlReplayReader;

impl JsonlReplayReader {
    pub fn read_trajectories(path: &Path) -> Result<Vec<ReplayTrajectory>, ReplayInfraError> {
        let mut seen_step_ids = BTreeSet::<String>::new();
        read_jsonl(path)?
            .into_iter()
            .enumerate()
            .map(|(index, value)| {
                let location = format!("{} line {}", path.display(), index + 1);
                let trajectory: ReplayTrajectory =
                    serde_json::from_value(value).map_err(|error| {
                        ReplayInfraError::new(format!(
                            "failed to parse trajectory {location}: {error}"
                        ))
                    })?;
                validate_trajectory_input(&trajectory, &location, &mut seen_step_ids)?;
                Ok(trajectory)
            })
            .collect()
    }

    pub fn read_predictions(
        path: &Path,
    ) -> Result<BTreeMap<String, VecDeque<ReplayPrediction>>, ReplayInfraError> {
        let mut predictions = BTreeMap::new();
        let mut seen_step_ids = BTreeSet::<String>::new();
        for (index, value) in read_jsonl(path)?.into_iter().enumerate() {
            let location = format!("{}:{}", path.display(), index + 1);
            let step_id = value
                .get("step_id")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    ReplayInfraError::new(format!(
                        "{location} missing required string field `step_id`"
                    ))
                })?;
            if !seen_step_ids.insert(step_id.to_string()) {
                return Err(ReplayInfraError::new(format!(
                    "{location} duplicate prediction step_id `{step_id}`; MCP replay requires unique prediction ids"
                )));
            }
            let action = value
                .get("action")
                .or_else(|| value.get("target_action"))
                .cloned()
                .ok_or_else(|| {
                    ReplayInfraError::new(format!("{location} missing `action` or `target_action`"))
                })?;
            predictions
                .entry(step_id.to_string())
                .or_insert_with(VecDeque::new)
                .push_back(ReplayPrediction { action });
        }
        Ok(predictions)
    }
}

pub struct ReplayOutputWriter;

impl ReplayOutputWriter {
    pub fn ensure_output_dir(output: &Path, force: bool) -> Result<(), ReplayInfraError> {
        if output.exists() {
            if !force {
                return Err(ReplayInfraError::new(format!(
                    "output directory already exists: {}; pass --force to replace generated files",
                    output.display()
                )));
            }
            if !output.is_dir() {
                return Err(ReplayInfraError::new(format!(
                    "output path exists and is not a directory: {}",
                    output.display()
                )));
            }
        } else {
            fs::create_dir_all(output).map_err(|error| {
                ReplayInfraError::new(format!(
                    "failed to create output directory {}: {error}",
                    output.display()
                ))
            })?;
        }
        Ok(())
    }

    pub fn write_report(output: &Path, report: &ReplayRunReport) -> Result<(), ReplayInfraError> {
        write_jsonl(
            &output.join("results.jsonl"),
            report.rows.iter().map(serde_json::to_value),
        )?;
        write_json_pretty(&output.join("summary.json"), &report.summary)?;
        Ok(())
    }
}

fn validate_trajectory_input(
    trajectory: &ReplayTrajectory,
    location: &str,
    seen_step_ids: &mut BTreeSet<String>,
) -> Result<(), ReplayInfraError> {
    if trajectory.step_id.trim().is_empty() {
        return Err(ReplayInfraError::new(format!(
            "{location}: step_id must not be empty"
        )));
    }
    if !seen_step_ids.insert(trajectory.step_id.clone()) {
        return Err(ReplayInfraError::new(format!(
            "{location}: duplicate step_id `{}`; MCP replay requires unique trajectory ids",
            trajectory.step_id
        )));
    }
    let allowed_for_mode = operator_allowed_tools_for_mode(&trajectory.mode)
        .ok_or_else(|| {
            ReplayInfraError::new(format!(
                "{location}: unsupported operator mode `{}`",
                trajectory.mode
            ))
        })?
        .into_iter()
        .collect::<BTreeSet<_>>();
    let mut seen_tools = BTreeSet::<String>::new();
    for tool in &trajectory.allowed_tools {
        if tool.trim().is_empty() {
            return Err(ReplayInfraError::new(format!(
                "{location}: allowed_tools must not contain empty values"
            )));
        }
        if !seen_tools.insert(tool.clone()) {
            return Err(ReplayInfraError::new(format!(
                "{location}: duplicate allowed_tools item `{tool}`"
            )));
        }
        if !allowed_for_mode.contains(tool) {
            return Err(ReplayInfraError::new(format!(
                "{location}: allowed_tools outside mode `{}`: {tool}",
                trajectory.mode
            )));
        }
    }
    Ok(())
}

fn read_jsonl(path: &Path) -> Result<Vec<Value>, ReplayInfraError> {
    let file = File::open(path).map_err(|error| {
        ReplayInfraError::new(format!("failed to open {}: {error}", path.display()))
    })?;
    let reader = BufReader::new(file);
    let mut values = Vec::new();
    for (index, line) in reader.lines().enumerate() {
        let line = line.map_err(|error| {
            ReplayInfraError::new(format!(
                "failed to read {} line {}: {error}",
                path.display(),
                index + 1
            ))
        })?;
        if line.trim().is_empty() {
            continue;
        }
        values.push(serde_json::from_str(&line).map_err(|error| {
            ReplayInfraError::new(format!(
                "failed to parse {} line {}: {error}",
                path.display(),
                index + 1
            ))
        })?);
    }
    Ok(values)
}

fn write_json_pretty(path: &Path, value: &impl Serialize) -> Result<(), ReplayInfraError> {
    let file = File::create(path).map_err(|error| {
        ReplayInfraError::new(format!("failed to create {}: {error}", path.display()))
    })?;
    let mut writer = BufWriter::new(file);
    serde_json::to_writer_pretty(&mut writer, value).map_err(|error| {
        ReplayInfraError::new(format!("failed to write {}: {error}", path.display()))
    })?;
    writer.write_all(b"\n").map_err(|error| {
        ReplayInfraError::new(format!("failed to write {}: {error}", path.display()))
    })?;
    writer.flush().map_err(|error| {
        ReplayInfraError::new(format!("failed to flush {}: {error}", path.display()))
    })?;
    Ok(())
}

fn write_jsonl<I>(path: &Path, values: I) -> Result<(), ReplayInfraError>
where
    I: IntoIterator<Item = Result<Value, serde_json::Error>>,
{
    let file = File::create(path).map_err(|error| {
        ReplayInfraError::new(format!("failed to create {}: {error}", path.display()))
    })?;
    let mut writer = BufWriter::new(file);
    for value in values {
        serde_json::to_writer(
            &mut writer,
            &value.map_err(|error| {
                ReplayInfraError::new(format!(
                    "failed to serialize JSONL value for {}: {error}",
                    path.display()
                ))
            })?,
        )
        .map_err(|error| {
            ReplayInfraError::new(format!("failed to write {}: {error}", path.display()))
        })?;
        writer.write_all(b"\n").map_err(|error| {
            ReplayInfraError::new(format!("failed to write {}: {error}", path.display()))
        })?;
    }
    writer.flush().map_err(|error| {
        ReplayInfraError::new(format!("failed to flush {}: {error}", path.display()))
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::json;

    use super::*;

    #[test]
    fn read_predictions_rejects_duplicate_step_ids() {
        let path = std::env::temp_dir().join(format!(
            "operator-replay-duplicate-predictions-{}.jsonl",
            std::process::id()
        ));
        fs::write(
            &path,
            r#"{"step_id":"s1","action":{"type":"stop","answer_policy":"evidence_or_unknown","final_refs":[],"reason":"done"}}"#
                .to_string()
                + "\n"
                + r#"{"step_id":"s1","action":{"type":"stop","answer_policy":"evidence_or_unknown","final_refs":[],"reason":"done"}}"#
                + "\n",
        )
        .expect("write fixture");

        let error = JsonlReplayReader::read_predictions(&path)
            .expect_err("duplicate prediction ids must fail fast");
        let _ = fs::remove_file(&path);

        assert!(error.to_string().contains("duplicate prediction step_id"));
    }

    #[test]
    fn read_trajectories_rejects_allowed_tools_outside_mode() {
        let path = std::env::temp_dir().join(format!(
            "operator-replay-bad-mode-tools-{}.jsonl",
            std::process::id()
        ));
        let row = json!({
            "step_id": "s1",
            "about": "about:1",
            "mode": "read",
            "task_family": "test",
            "allowed_tools": ["kernel_near", "kernel_write_memory"],
            "observed_outcome": null
        });
        fs::write(&path, format!("{row}\n")).expect("write fixture");

        let error = JsonlReplayReader::read_trajectories(&path)
            .expect_err("MCP replay trajectories must keep tools inside mode");
        let _ = fs::remove_file(&path);

        assert!(
            error
                .to_string()
                .contains("allowed_tools outside mode `read`: kernel_write_memory")
        );
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
            "underpass-operator-replay-infra must stay independent from kernel crates"
        );
    }
}
