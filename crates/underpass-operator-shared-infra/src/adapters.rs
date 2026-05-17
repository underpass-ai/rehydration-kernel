use underpass_operator_shared_application::ReadTrainingDatasetError;
use underpass_operator_shared_application::ports::TrainingTrajectoryReader;
use underpass_operator_shared_domain::TrainingTrajectory;

#[derive(Debug, Clone)]
pub struct InMemoryTrainingTrajectoryReader {
    trajectories: Vec<TrainingTrajectory>,
}

impl InMemoryTrainingTrajectoryReader {
    pub fn new(trajectories: Vec<TrainingTrajectory>) -> Self {
        Self { trajectories }
    }
}

impl TrainingTrajectoryReader for InMemoryTrainingTrajectoryReader {
    fn read_training_trajectories(
        &self,
    ) -> Result<Vec<TrainingTrajectory>, ReadTrainingDatasetError> {
        Ok(self.trajectories.clone())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use underpass_operator_shared_application::ValidateTrainingDatasetUseCase;

    use crate::mappers::TrainingTrajectoryMapper;

    use super::*;

    #[test]
    fn adapter_feeds_training_preflight_use_case() {
        let trajectory = TrainingTrajectoryMapper::from_json(json!({
            "step_id": "step-1",
            "about": "about:incident-1",
            "mode": "read",
            "task_family": "contract.read.near",
            "allowed_tools": ["kernel_near"],
            "visible_state": {},
            "target_action": {
                "type": "tool_call",
                "tool": "kernel_near",
                "arguments": {}
            }
        }))
        .expect("json maps to domain");

        let reader = InMemoryTrainingTrajectoryReader::new(vec![trajectory]);
        let report = ValidateTrainingDatasetUseCase::new(reader)
            .execute()
            .expect("use case should pass");

        assert_eq!(report.total().as_usize(), 1);
    }
}
