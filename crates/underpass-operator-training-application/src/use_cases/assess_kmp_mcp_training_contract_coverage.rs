use underpass_operator_training_domain::TrainingContractCoverageReport;

use crate::ports::TrainingTrajectoryReader;
use crate::{TrainingApplicationError, TrainingApplicationResult};

pub struct AssessKmpMcpTrainingContractCoverageUseCase<R> {
    reader: R,
}

impl<R> AssessKmpMcpTrainingContractCoverageUseCase<R>
where
    R: TrainingTrajectoryReader,
{
    pub fn new(reader: R) -> Self {
        Self { reader }
    }

    pub fn execute(&self) -> TrainingApplicationResult<TrainingContractCoverageReport> {
        let trajectories = self
            .reader
            .read_training_trajectories()
            .map_err(TrainingApplicationError::ReadTrainingTrajectories)?;
        Ok(TrainingContractCoverageReport::from_trajectories(
            &trajectories,
        )?)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use underpass_operator_shared_domain::{
        AboutId, ActionArguments, AllowedTools, KernelTool, KmpMcpCapability, OperatorAction,
        OperatorMode, StepId, TaskFamily, TrainingTrajectory, VisibleState,
    };

    use super::*;
    use crate::ReadTrainingRunPlanError;

    struct StaticReader {
        trajectories: Vec<TrainingTrajectory>,
    }

    impl TrainingTrajectoryReader for StaticReader {
        fn read_training_trajectories(
            &self,
        ) -> Result<Vec<TrainingTrajectory>, ReadTrainingRunPlanError> {
            Ok(self.trajectories.clone())
        }
    }

    #[test]
    fn assesses_training_contract_capability_coverage() {
        let report = AssessKmpMcpTrainingContractCoverageUseCase::new(StaticReader {
            trajectories: vec![trajectory()],
        })
        .execute()
        .expect("report");

        assert!(
            report
                .by_capability()
                .contains_key(&KmpMcpCapability::from_tool(KernelTool::Inspect))
        );
    }

    fn trajectory() -> TrainingTrajectory {
        let mode = OperatorMode::Read;
        TrainingTrajectory::new(
            StepId::parse("step-1").expect("step"),
            AboutId::parse("about-1").expect("about"),
            mode,
            TaskFamily::parse("read.inspect").expect("task"),
            AllowedTools::parse(mode, vec![KernelTool::Inspect]).expect("tools"),
            VisibleState::parse(json!({})).expect("visible"),
            OperatorAction::tool_call(
                KernelTool::Inspect,
                ActionArguments::parse(json!({ "ref": "node-1" })).expect("arguments"),
            ),
        )
        .expect("trajectory")
    }
}
