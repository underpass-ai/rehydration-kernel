use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use serde_json::Value;
use underpass_operator_evaluation_domain::PolicyEvalTrajectory;
use underpass_operator_shared_domain::operator_allowed_tools_for_mode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyTrajectoryJsonlFormat {
    RawTrajectories,
    ModelFacingEval,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyEvalJsonlError {
    message: String,
}

impl PolicyEvalJsonlError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for PolicyEvalJsonlError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for PolicyEvalJsonlError {}

pub struct JsonlPolicyEvalReader;

impl JsonlPolicyEvalReader {
    pub fn read_trajectories(
        path: &Path,
        format: PolicyTrajectoryJsonlFormat,
    ) -> Result<Vec<PolicyEvalTrajectory>, PolicyEvalJsonlError> {
        match format {
            PolicyTrajectoryJsonlFormat::RawTrajectories => read_raw_trajectories(path),
            PolicyTrajectoryJsonlFormat::ModelFacingEval => read_model_facing_eval(path),
        }
    }

    pub fn read_predictions(path: &Path) -> Result<BTreeMap<String, Value>, PolicyEvalJsonlError> {
        read_predictions(path)
    }
}

fn read_raw_trajectories(path: &Path) -> Result<Vec<PolicyEvalTrajectory>, PolicyEvalJsonlError> {
    let mut seen_step_ids = BTreeSet::<String>::new();
    read_jsonl(path)?
        .into_iter()
        .enumerate()
        .map(|(index, value)| {
            let location = format!("{}:{}", path.display(), index + 1);
            let step_id = required_string(&value, "step_id", &location)?.to_string();
            if !seen_step_ids.insert(step_id.clone()) {
                return Err(PolicyEvalJsonlError::new(format!(
                    "{location} duplicate step_id `{step_id}`; policy evaluation requires unique trajectory step ids"
                )));
            }
            let mode = required_string(&value, "mode", &location)?.to_string();
            let allowed_tools = required_string_set(&value, "allowed_tools", &location)?;
            validate_allowed_tools_for_mode(&mode, &allowed_tools, &location)?;
            Ok(PolicyEvalTrajectory {
                step_id,
                about: required_string(&value, "about", &location)?.to_string(),
                mode,
                task_family: required_string(&value, "task_family", &location)?.to_string(),
                allowed_tools: Some(allowed_tools),
                visible_state: value.get("visible_state").cloned().ok_or_else(|| {
                    PolicyEvalJsonlError::new(format!(
                        "{location} missing required field `visible_state`"
                    ))
                })?,
                target_action: value.get("target_action").cloned().ok_or_else(|| {
                    PolicyEvalJsonlError::new(format!(
                        "{location} missing required field `target_action`"
                    ))
                })?,
            })
        })
        .collect()
}

fn read_model_facing_eval(path: &Path) -> Result<Vec<PolicyEvalTrajectory>, PolicyEvalJsonlError> {
    let mut seen_step_ids = BTreeSet::<String>::new();
    read_jsonl(path)?
        .into_iter()
        .enumerate()
        .map(|(index, value)| {
            let location = format!("{}:{}", path.display(), index + 1);
            let step_id = value
                .get("step_id")
                .or_else(|| value.get("id"))
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    PolicyEvalJsonlError::new(format!(
                        "{location} missing required string field `step_id` or `id`"
                    ))
                })?
                .to_string();
            if !seen_step_ids.insert(step_id.clone()) {
                return Err(PolicyEvalJsonlError::new(format!(
                    "{location} duplicate step_id `{step_id}`; policy evaluation requires unique model-facing eval ids"
                )));
            }
            let messages = value
                .get("messages")
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    PolicyEvalJsonlError::new(format!(
                        "{location} missing required array field `messages`"
                    ))
                })?;
            validate_model_facing_messages(messages, &location)?;
            let user_message = message_content_at(messages, 1, &location)?;
            let assistant_message = message_content_at(messages, 2, &location)?;
            let user_value: Value = serde_json::from_str(user_message).map_err(|error| {
                PolicyEvalJsonlError::new(format!(
                    "{location} user message content is not valid JSON: {error}"
                ))
            })?;
            let assistant_value: Value =
                serde_json::from_str(assistant_message).map_err(|error| {
                    PolicyEvalJsonlError::new(format!(
                        "{location} assistant message content is not valid JSON: {error}"
                    ))
                })?;
            let target_action = assistant_value.get("action").cloned().ok_or_else(|| {
                PolicyEvalJsonlError::new(format!("{location} assistant JSON missing `action`"))
            })?;
            let mode = optional_string(&value, "mode")
                .or_else(|| optional_string(&user_value, "mode"))
                .ok_or_else(|| {
                    PolicyEvalJsonlError::new(format!(
                        "{location} missing required string field `mode`"
                    ))
                })?
                .to_string();
            let allowed_tools = required_string_set(&user_value, "allowed_tools", &location)?;
            validate_allowed_tools_for_mode(&mode, &allowed_tools, &location)?;
            Ok(PolicyEvalTrajectory {
                step_id,
                about: required_string(&user_value, "about", &location)?.to_string(),
                mode,
                task_family: optional_string(&value, "task_family")
                    .or_else(|| optional_string(&user_value, "task_family"))
                    .ok_or_else(|| {
                        PolicyEvalJsonlError::new(format!(
                            "{location} missing required string field `task_family`"
                        ))
                    })?
                    .to_string(),
                allowed_tools: Some(allowed_tools),
                visible_state: user_value.get("visible_state").cloned().ok_or_else(|| {
                    PolicyEvalJsonlError::new(format!(
                        "{location} user JSON missing required field `visible_state`"
                    ))
                })?,
                target_action,
            })
        })
        .collect()
}

fn read_predictions(path: &Path) -> Result<BTreeMap<String, Value>, PolicyEvalJsonlError> {
    let mut predictions = BTreeMap::new();
    for (index, value) in read_jsonl(path)?.into_iter().enumerate() {
        let location = format!("{}:{}", path.display(), index + 1);
        let step_id = required_string(&value, "step_id", &location)?;
        let action = value
            .get("action")
            .or_else(|| value.get("target_action"))
            .cloned()
            .ok_or_else(|| {
                PolicyEvalJsonlError::new(format!("{location} missing `action` or `target_action`"))
            })?;
        if predictions.insert(step_id.to_string(), action).is_some() {
            return Err(PolicyEvalJsonlError::new(format!(
                "{location} duplicate prediction step_id `{step_id}`; policy evaluation requires unique prediction step ids"
            )));
        }
    }
    Ok(predictions)
}

fn read_jsonl(path: &Path) -> Result<Vec<Value>, PolicyEvalJsonlError> {
    let file = File::open(path).map_err(|error| {
        PolicyEvalJsonlError::new(format!("failed to open {}: {error}", path.display()))
    })?;
    let reader = BufReader::new(file);
    let mut values = Vec::new();
    for (index, line) in reader.lines().enumerate() {
        let line = line.map_err(|error| {
            PolicyEvalJsonlError::new(format!(
                "failed to read {} line {}: {error}",
                path.display(),
                index + 1
            ))
        })?;
        if line.trim().is_empty() {
            continue;
        }
        values.push(serde_json::from_str(&line).map_err(|error| {
            PolicyEvalJsonlError::new(format!(
                "failed to parse {} line {}: {error}",
                path.display(),
                index + 1
            ))
        })?);
    }
    Ok(values)
}

fn required_string<'a>(
    value: &'a Value,
    field: &str,
    location: &str,
) -> Result<&'a str, PolicyEvalJsonlError> {
    value.get(field).and_then(Value::as_str).ok_or_else(|| {
        PolicyEvalJsonlError::new(format!(
            "{location} missing required string field `{field}`"
        ))
    })
}

fn optional_string<'a>(value: &'a Value, field: &str) -> Option<&'a str> {
    value.get(field).and_then(Value::as_str)
}

fn required_string_set(
    value: &Value,
    field: &str,
    location: &str,
) -> Result<BTreeSet<String>, PolicyEvalJsonlError> {
    let values = value.get(field).and_then(Value::as_array).ok_or_else(|| {
        PolicyEvalJsonlError::new(format!("{location} missing required array field `{field}`"))
    })?;
    let mut result = BTreeSet::new();
    for (index, item) in values.iter().enumerate() {
        let item = item
            .as_str()
            .filter(|item| !item.is_empty())
            .ok_or_else(|| {
                PolicyEvalJsonlError::new(format!(
                    "{location} `{field}` item {index} must be a non-empty string"
                ))
            })?;
        if !result.insert(item.to_string()) {
            return Err(PolicyEvalJsonlError::new(format!(
                "{location} duplicate `{field}` item `{item}`"
            )));
        }
    }
    Ok(result)
}

fn validate_allowed_tools_for_mode(
    mode: &str,
    allowed_tools: &BTreeSet<String>,
    location: &str,
) -> Result<(), PolicyEvalJsonlError> {
    let allowed_for_mode = operator_allowed_tools_for_mode(mode)
        .ok_or_else(|| {
            PolicyEvalJsonlError::new(format!("{location} unsupported operator mode `{mode}`"))
        })?
        .into_iter()
        .collect::<BTreeSet<_>>();
    let unsupported = allowed_tools
        .iter()
        .filter(|tool| !allowed_for_mode.contains(*tool))
        .cloned()
        .collect::<Vec<_>>();
    if unsupported.is_empty() {
        Ok(())
    } else {
        Err(PolicyEvalJsonlError::new(format!(
            "{location} allowed_tools outside mode `{mode}`: {}",
            unsupported.join(",")
        )))
    }
}

fn validate_model_facing_messages(
    messages: &[Value],
    location: &str,
) -> Result<(), PolicyEvalJsonlError> {
    if messages.len() != 3 {
        return Err(PolicyEvalJsonlError::new(format!(
            "{location} expected exactly 3 messages"
        )));
    }
    for (index, expected_role) in ["system", "user", "assistant"].iter().enumerate() {
        let actual_role = messages[index].get("role").and_then(Value::as_str);
        if actual_role != Some(*expected_role) {
            return Err(PolicyEvalJsonlError::new(format!(
                "{location} expected message roles system/user/assistant, got role `{}` at index {index}",
                actual_role.unwrap_or("<missing>")
            )));
        }
        message_content_at(messages, index, location)?;
    }
    Ok(())
}

fn message_content_at<'a>(
    messages: &'a [Value],
    index: usize,
    location: &str,
) -> Result<&'a str, PolicyEvalJsonlError> {
    messages[index]
        .get("content")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            PolicyEvalJsonlError::new(format!(
                "{location} message {index} missing string `content`"
            ))
        })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::json;

    use super::*;

    #[test]
    fn raw_trajectory_reader_rejects_duplicate_step_ids() {
        let path = std::env::temp_dir().join(format!(
            "policy-eval-infra-duplicate-trajectories-{}.jsonl",
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
        fs::write(&path, format!("{row}\n{row}\n")).expect("write fixture");

        let error = JsonlPolicyEvalReader::read_trajectories(
            &path,
            PolicyTrajectoryJsonlFormat::RawTrajectories,
        )
        .expect_err("duplicate step ids should fail");
        let _ = fs::remove_file(&path);

        assert!(error.to_string().contains("duplicate step_id"));
    }

    #[test]
    fn model_facing_reader_uses_assistant_action_as_target() {
        let path = std::env::temp_dir().join(format!(
            "policy-eval-infra-model-facing-{}.jsonl",
            std::process::id()
        ));
        let user = json!({
            "about": "about_0001",
            "mode": "read",
            "task_family": "conformance.read.near",
            "allowed_tools": ["kernel_near"],
            "visible_state": { "current_ref": "ref_0001" }
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
        fs::write(&path, format!("{row}\n")).expect("write fixture");

        let trajectories = JsonlPolicyEvalReader::read_trajectories(
            &path,
            PolicyTrajectoryJsonlFormat::ModelFacingEval,
        )
        .expect("trajectories");
        let _ = fs::remove_file(&path);

        assert_eq!(trajectories.len(), 1);
        assert_eq!(trajectories[0].step_id, "s1");
        assert_eq!(trajectories[0].target_action, action);
    }
}
