use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::OperatorActionDto;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawTrajectoryDto {
    pub step_id: String,
    pub about: String,
    pub mode: String,
    pub task_family: String,
    pub allowed_tools: Vec<String>,
    pub visible_state: Value,
    pub target_action: OperatorActionDto,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn deserializes_raw_trajectory_dto() {
        let trajectory: RawTrajectoryDto = serde_json::from_value(json!({
            "step_id": "step-1",
            "about": "about:incident-1",
            "mode": "read",
            "task_family": "contract.read.near",
            "allowed_tools": ["kernel_near"],
            "visible_state": {},
            "target_action": {
                "type": "tool_call",
                "tool": "kernel_near",
                "arguments": {
                    "around": { "ref": "node-1" }
                }
            }
        }))
        .expect("trajectory dto should deserialize");

        assert_eq!(trajectory.step_id, "step-1");
    }

    #[test]
    fn rejects_unknown_trajectory_fields() {
        let error = serde_json::from_value::<RawTrajectoryDto>(json!({
            "step_id": "step-1",
            "about": "about:incident-1",
            "mode": "read",
            "task_family": "contract.read",
            "allowed_tools": [],
            "visible_state": {},
            "target_action": {
                "type": "stop",
                "answer_policy": "evidence_or_unknown",
                "final_refs": [],
                "reason": "done"
            },
            "fallback": true
        }))
        .expect_err("dto must reject unknown fields");

        assert!(error.to_string().contains("unknown field"));
    }
}
