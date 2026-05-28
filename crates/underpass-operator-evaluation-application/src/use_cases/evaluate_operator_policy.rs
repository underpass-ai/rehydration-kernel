use underpass_operator_evaluation_domain::{PolicyEvalRequest, PolicyEvalResult, PolicyEvaluator};

pub struct EvaluateOperatorPolicyUseCase;

impl EvaluateOperatorPolicyUseCase {
    pub fn new() -> Self {
        Self
    }

    pub fn execute(&self, request: PolicyEvalRequest) -> Result<PolicyEvalResult, String> {
        PolicyEvaluator::evaluate(request)
    }
}

impl Default for EvaluateOperatorPolicyUseCase {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use underpass_operator_evaluation_domain::{PolicyEvalBaseline, PolicyEvalTrajectory};

    use super::*;

    #[test]
    fn evaluates_policy_request_through_application_use_case() {
        let action = json!({
            "type": "tool_call",
            "tool": "kernel_inspect",
            "arguments": {
                "ref": "node:1",
                "include": {
                    "details": true,
                    "incoming": true,
                    "outgoing": true,
                    "raw": false
                }
            }
        });
        let request = PolicyEvalRequest {
            target_source: "raw_trajectories".to_string(),
            trajectories_label: "trajectories.jsonl".to_string(),
            predictions_label: None,
            predictor: "baseline:oracle".to_string(),
            generated_at_unix_seconds: 1,
            baseline: PolicyEvalBaseline::Oracle,
            resolve_prepared_payloads: false,
            trajectories: vec![PolicyEvalTrajectory {
                step_id: "s1".to_string(),
                about: "about:1".to_string(),
                mode: "read".to_string(),
                task_family: "conformance.inspect".to_string(),
                allowed_tools: None,
                visible_state: json!({}),
                target_action: action,
            }],
            predictions: None,
        };

        let result = EvaluateOperatorPolicyUseCase::new()
            .execute(request)
            .expect("policy result");

        assert_eq!(result.summary.counts.exact_action_correct, 1);
    }
}
