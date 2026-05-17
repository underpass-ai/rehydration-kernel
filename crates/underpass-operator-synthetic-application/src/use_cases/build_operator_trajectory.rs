use underpass_operator_shared_domain::TrainingTrajectory;
use underpass_operator_synthetic_domain::{OperatorTrajectoryBuildSpec, SyntheticCaseSpec};

use crate::SyntheticApplicationResult;

pub struct BuildOperatorTrajectoryUseCase;

impl BuildOperatorTrajectoryUseCase {
    pub fn execute(
        case: &SyntheticCaseSpec,
        spec: OperatorTrajectoryBuildSpec,
    ) -> SyntheticApplicationResult<TrainingTrajectory> {
        Ok(case.build_trajectory(spec)?)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use underpass_operator_shared_domain::{
        AboutId, ActionArguments, KernelTool, OperatorAction, OperatorMode, PositiveCount, StepId,
        SyntheticCaseId, TaskFamily, VisibleState,
    };
    use underpass_operator_synthetic_domain::{OperatorTrajectoryBuildSpec, SyntheticCaseSpec};

    use super::*;

    #[test]
    fn builds_a_canonical_trajectory_for_a_case() {
        let case = SyntheticCaseSpec::new(
            SyntheticCaseId::parse("case-1").expect("case"),
            OperatorMode::Read,
            TaskFamily::parse("read.inspect").expect("task"),
            KernelTool::Inspect,
            PositiveCount::parse(1, "minimum").expect("minimum"),
        )
        .expect("case");

        let trajectory = BuildOperatorTrajectoryUseCase::execute(
            &case,
            OperatorTrajectoryBuildSpec::new(
                StepId::parse("step-1").expect("step"),
                AboutId::parse("about-1").expect("about"),
                VisibleState::parse(json!({ "cursor": { "ref": "node-1" } })).expect("visible"),
                OperatorAction::tool_call(
                    KernelTool::Inspect,
                    ActionArguments::parse(json!({ "ref": "node-1" })).expect("arguments"),
                ),
            ),
        )
        .expect("trajectory");

        assert_eq!(trajectory.step_id().as_str(), "step-1");
        assert_eq!(
            trajectory.allowed_tools().as_slice(),
            OperatorMode::Read.allowed_tools()
        );
    }
}
