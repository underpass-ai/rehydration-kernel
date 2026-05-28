use underpass_operator_shared_domain::TrainingDatasetPreflightReport;

use crate::ApplicationResult;
use crate::ports::TrainingTrajectoryReader;

pub struct ValidateTrainingDatasetUseCase<R> {
    reader: R,
}

impl<R> ValidateTrainingDatasetUseCase<R>
where
    R: TrainingTrajectoryReader,
{
    pub fn new(reader: R) -> Self {
        Self { reader }
    }

    pub fn execute(&self) -> ApplicationResult<TrainingDatasetPreflightReport> {
        let trajectories = self.reader.read_training_trajectories()?;
        Ok(TrainingDatasetPreflightReport::from_trajectories(
            &trajectories,
        )?)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use underpass_operator_shared_domain::{
        AboutId, ActionArguments, AllowedTools, ExampleCount, KernelTool, OperatorAction,
        OperatorMode, StepId, TaskFamily, TrainingTrajectory, VisibleState,
    };

    use crate::{ApplicationError, ReadTrainingDatasetError};

    use super::*;

    struct StaticReader {
        trajectories: Vec<TrainingTrajectory>,
    }

    impl TrainingTrajectoryReader for StaticReader {
        fn read_training_trajectories(
            &self,
        ) -> Result<Vec<TrainingTrajectory>, ReadTrainingDatasetError> {
            Ok(self.trajectories.clone())
        }
    }

    #[test]
    fn validates_dataset_and_builds_preflight_report() {
        let reader = StaticReader {
            trajectories: vec![trajectory("step-1", KernelTool::Near)],
        };

        let report = ValidateTrainingDatasetUseCase::new(reader)
            .execute()
            .expect("dataset should validate");

        assert_eq!(report.total().as_usize(), 1);
        assert_eq!(
            report.by_mode().get(&OperatorMode::Read).copied(),
            Some(ExampleCount::from_usize(1))
        );
        assert_eq!(
            report.by_target_tool().get(&KernelTool::Near).copied(),
            Some(ExampleCount::from_usize(1))
        );
    }

    #[test]
    fn fails_fast_on_duplicate_step_id() {
        let reader = StaticReader {
            trajectories: vec![
                trajectory("step-1", KernelTool::Near),
                trajectory("step-1", KernelTool::Inspect),
            ],
        };

        let error = ValidateTrainingDatasetUseCase::new(reader)
            .execute()
            .expect_err("duplicate step id must fail");

        assert_eq!(
            error,
            ApplicationError::Domain(
                underpass_operator_shared_domain::DomainError::DuplicateStepId {
                    step_id: "step-1".to_string()
                }
            )
        );
    }

    fn trajectory(step_id: &str, target_tool: KernelTool) -> TrainingTrajectory {
        let mode = OperatorMode::Read;
        let allowed_tools = AllowedTools::parse(mode, vec![KernelTool::Near, KernelTool::Inspect])
            .expect("valid tools");
        let action = OperatorAction::tool_call(
            target_tool,
            ActionArguments::parse(json!({ "ref": "node-1" })).expect("valid args"),
        );
        TrainingTrajectory::new(
            StepId::parse(step_id).expect("step"),
            AboutId::parse("about-1").expect("about"),
            mode,
            TaskFamily::parse("read.near").expect("task"),
            allowed_tools,
            VisibleState::parse(json!({ "cursor": { "ref": "node-1" } })).expect("visible state"),
            action,
        )
        .expect("valid trajectory")
    }
}
