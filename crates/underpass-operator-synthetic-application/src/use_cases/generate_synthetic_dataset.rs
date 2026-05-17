use underpass_operator_synthetic_domain::{
    SyntheticDataset, SyntheticDatasetBlueprint, SyntheticDatasetGenerationReport,
};

use crate::SyntheticApplicationResult;
use crate::ports::SyntheticCaseGenerator;

pub struct GenerateSyntheticDatasetUseCase<G> {
    generator: G,
}

impl<G> GenerateSyntheticDatasetUseCase<G>
where
    G: SyntheticCaseGenerator,
{
    pub fn new(generator: G) -> Self {
        Self { generator }
    }

    pub fn execute(
        &self,
        blueprint: &SyntheticDatasetBlueprint,
    ) -> SyntheticApplicationResult<SyntheticDatasetGenerationReport> {
        let mut trajectories = Vec::new();
        let mut case_metrics = Vec::new();

        for case in blueprint.cases() {
            let generated = self.generator.generate_case(case)?;
            for trajectory in &generated {
                case.validate_trajectory(trajectory)?;
            }
            let actual = generated.len();
            let metric = case.generation_metric(
                underpass_operator_shared_domain::ExampleCount::from_usize(actual),
            )?;
            case_metrics.push(metric);
            trajectories.extend(generated);
        }

        let dataset = SyntheticDataset::new(blueprint.dataset_id().clone(), trajectories)?;
        Ok(SyntheticDatasetGenerationReport::new(
            dataset,
            case_metrics,
        )?)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use underpass_operator_shared_domain::{
        AboutId, ActionArguments, AllowedTools, KernelTool, OperatorAction, OperatorMode,
        PositiveCount, StepId, SyntheticCaseId, TaskFamily, TrainingTrajectory, VisibleState,
    };
    use underpass_operator_synthetic_domain::{SyntheticCaseSpec, SyntheticDatasetBlueprint};

    use super::*;
    use crate::{GenerateSyntheticCaseError, SyntheticApplicationError};

    struct StaticGenerator {
        generated: Vec<TrainingTrajectory>,
    }

    impl SyntheticCaseGenerator for StaticGenerator {
        fn generate_case(
            &self,
            _spec: &SyntheticCaseSpec,
        ) -> Result<Vec<TrainingTrajectory>, GenerateSyntheticCaseError> {
            Ok(self.generated.clone())
        }
    }

    #[test]
    fn generates_dataset_from_blueprint() {
        let use_case = GenerateSyntheticDatasetUseCase::new(StaticGenerator {
            generated: vec![trajectory("step-1")],
        });
        let blueprint = blueprint(1);

        let report = use_case.execute(&blueprint).expect("dataset");

        assert_eq!(report.dataset().trajectories().len(), 1);
        assert_eq!(report.total_generated().as_usize(), 1);
    }

    #[test]
    fn fails_fast_when_case_does_not_meet_minimum() {
        let use_case = GenerateSyntheticDatasetUseCase::new(StaticGenerator { generated: vec![] });
        let blueprint = blueprint(1);

        let error = use_case
            .execute(&blueprint)
            .expect_err("underproduced case must fail");

        assert_eq!(
            error,
            SyntheticApplicationError::Domain(
                underpass_operator_shared_domain::DomainError::CountBelowMinimum {
                    context: "synthetic_case.case-1.generated".to_string(),
                    minimum: 1,
                    actual: 0
                }
            )
        );
    }

    #[test]
    fn fails_fast_when_generator_returns_trajectory_for_another_case() {
        let use_case = GenerateSyntheticDatasetUseCase::new(StaticGenerator {
            generated: vec![trajectory_for_tool("step-1", KernelTool::Near)],
        });
        let blueprint = blueprint(1);

        let error = use_case
            .execute(&blueprint)
            .expect_err("wrong generated target must fail");

        assert_eq!(
            error,
            SyntheticApplicationError::Domain(
                underpass_operator_shared_domain::DomainError::TrajectoryCaseMismatch {
                    field: "target_action.tool".to_string(),
                    expected: "kernel_inspect".to_string(),
                    actual: "kernel_near".to_string()
                }
            )
        );
    }

    fn blueprint(minimum: usize) -> SyntheticDatasetBlueprint {
        let case = SyntheticCaseSpec::new(
            SyntheticCaseId::parse("case-1").expect("case"),
            OperatorMode::Read,
            TaskFamily::parse("read.inspect").expect("task"),
            KernelTool::Inspect,
            PositiveCount::parse(minimum, "minimum_examples").expect("minimum"),
        )
        .expect("case spec");

        SyntheticDatasetBlueprint::new(
            underpass_operator_shared_domain::DatasetId::parse("dataset-1").expect("dataset"),
            vec![case],
        )
        .expect("blueprint")
    }

    fn trajectory(step_id: &str) -> TrainingTrajectory {
        trajectory_for_tool(step_id, KernelTool::Inspect)
    }

    fn trajectory_for_tool(step_id: &str, tool: KernelTool) -> TrainingTrajectory {
        let mode = OperatorMode::Read;
        let allowed_tools = AllowedTools::all_for_mode(mode);
        let arguments = match tool {
            KernelTool::Near => json!({ "around": { "ref": "node-1" } }),
            KernelTool::Inspect => json!({ "ref": "node-1" }),
            _ => json!({}),
        };
        TrainingTrajectory::new(
            StepId::parse(step_id).expect("step"),
            AboutId::parse("about-1").expect("about"),
            mode,
            TaskFamily::parse("read.inspect").expect("task"),
            allowed_tools,
            VisibleState::parse(json!({ "cursor": { "ref": "node-1" } })).expect("visible state"),
            OperatorAction::tool_call(tool, ActionArguments::parse(arguments).expect("arguments")),
        )
        .expect("trajectory")
    }
}
