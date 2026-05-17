use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum OperatorActionDto {
    #[serde(rename = "tool_call")]
    ToolCall(ToolCallActionDto),
    #[serde(rename = "prepared_tool_call")]
    PreparedToolCall(PreparedToolCallActionDto),
    #[serde(rename = "stop")]
    Stop(StopActionDto),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolCallActionDto {
    pub tool: String,
    pub arguments: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparedToolCallActionDto {
    pub tool: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StopActionDto {
    pub answer_policy: String,
    pub final_refs: Vec<String>,
    pub reason: String,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn deserializes_tool_call_shape() {
        let action: OperatorActionDto = serde_json::from_value(json!({
            "type": "tool_call",
            "tool": "kernel_near",
            "arguments": {
                "around": { "ref": "node-1" }
            }
        }))
        .expect("tool call dto should deserialize");

        assert_eq!(
            action,
            OperatorActionDto::ToolCall(ToolCallActionDto {
                tool: "kernel_near".to_string(),
                arguments: json!({ "around": { "ref": "node-1" } })
            })
        );
    }

    #[test]
    fn rejects_unknown_tool_call_fields() {
        let error = serde_json::from_value::<OperatorActionDto>(json!({
            "type": "tool_call",
            "tool": "kernel_near",
            "arguments": {},
            "fallback": true
        }))
        .expect_err("dto must reject unexpected fields");

        assert!(error.to_string().contains("unknown field"));
    }
}
