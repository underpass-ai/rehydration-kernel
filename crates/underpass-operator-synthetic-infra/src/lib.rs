use std::collections::BTreeMap;

use underpass_operator_shared_domain::TrainingTrajectory;
use underpass_operator_shared_infra::mappers::TrainingTrajectoryMapper;
use underpass_operator_synthetic_application::{
    GenerateSyntheticCaseError, ports::SyntheticCaseGenerator,
};
use underpass_operator_synthetic_domain::SyntheticCaseSpec;

pub trait SyntheticCaseTeacher {
    fn generate_candidate_rows(
        &self,
        spec: &SyntheticCaseSpec,
    ) -> Result<Vec<serde_json::Value>, GenerateSyntheticCaseError>;
}

pub struct TeacherSyntheticCaseGenerator<T> {
    teacher: T,
}

impl<T> TeacherSyntheticCaseGenerator<T> {
    pub fn new(teacher: T) -> Self {
        Self { teacher }
    }
}

impl<T> SyntheticCaseGenerator for TeacherSyntheticCaseGenerator<T>
where
    T: SyntheticCaseTeacher,
{
    fn generate_case(
        &self,
        spec: &SyntheticCaseSpec,
    ) -> Result<Vec<TrainingTrajectory>, GenerateSyntheticCaseError> {
        let rows = self.teacher.generate_candidate_rows(spec)?;
        rows.into_iter()
            .enumerate()
            .map(|(index, row)| {
                TrainingTrajectoryMapper::from_json(row).map_err(|error| {
                    GenerateSyntheticCaseError::new(format!(
                        "teacher candidate `{}` for synthetic case `{}` is invalid: {error}",
                        index,
                        spec.case_id().as_str()
                    ))
                })
            })
            .collect()
    }
}

pub struct InMemorySyntheticCaseGenerator {
    cases: BTreeMap<String, Vec<TrainingTrajectory>>,
}

impl InMemorySyntheticCaseGenerator {
    pub fn new(cases: BTreeMap<String, Vec<TrainingTrajectory>>) -> Self {
        Self { cases }
    }
}

impl SyntheticCaseGenerator for InMemorySyntheticCaseGenerator {
    fn generate_case(
        &self,
        spec: &SyntheticCaseSpec,
    ) -> Result<Vec<TrainingTrajectory>, GenerateSyntheticCaseError> {
        self.cases
            .get(spec.case_id().as_str())
            .cloned()
            .ok_or_else(|| {
                GenerateSyntheticCaseError::new(format!(
                    "synthetic case `{}` is not registered",
                    spec.case_id().as_str()
                ))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use underpass_operator_shared_domain::{
        AboutId, ActionArguments, AllowedTools, DatasetId, KernelTool, OperatorAction,
        OperatorMode, PositiveCount, StepId, SyntheticCaseId, TaskFamily, TrainingTrajectory,
        VisibleState,
    };
    use underpass_operator_synthetic_application::GenerateSyntheticDatasetUseCase;
    use underpass_operator_synthetic_domain::{SyntheticCaseSpec, SyntheticDatasetBlueprint};

    struct StaticTeacher {
        rows: Vec<serde_json::Value>,
    }

    impl SyntheticCaseTeacher for StaticTeacher {
        fn generate_candidate_rows(
            &self,
            _spec: &SyntheticCaseSpec,
        ) -> Result<Vec<serde_json::Value>, GenerateSyntheticCaseError> {
            Ok(self.rows.clone())
        }
    }

    #[test]
    fn teacher_generator_maps_valid_candidate_rows_to_trajectories() {
        let generator = TeacherSyntheticCaseGenerator::new(StaticTeacher {
            rows: vec![trajectory_row("step-1")],
        });
        let case = synthetic_case(1);

        let generated = generator.generate_case(&case).expect("generated");

        assert_eq!(generated.len(), 1);
        assert_eq!(generated[0].step_id().as_str(), "step-1");
    }

    #[test]
    fn teacher_generator_fails_fast_on_invalid_candidate_row() {
        let generator = TeacherSyntheticCaseGenerator::new(StaticTeacher {
            rows: vec![json!({
                "step_id": "step-1",
                "about": "about:1"
            })],
        });
        let case = synthetic_case(1);

        let error = generator
            .generate_case(&case)
            .expect_err("invalid candidate row must fail");

        assert!(error.message().contains("teacher candidate `0`"));
        assert!(error.message().contains("synthetic case `case-1`"));
    }

    #[test]
    fn in_memory_generator_plugs_into_synthetic_use_case() {
        let generator = InMemorySyntheticCaseGenerator::new(BTreeMap::from([(
            "case-1".to_string(),
            vec![trajectory()],
        )]));
        let case = synthetic_case(1);
        let blueprint = SyntheticDatasetBlueprint::new(
            DatasetId::parse("dataset-1").expect("dataset"),
            vec![case],
        )
        .expect("blueprint");

        let report = GenerateSyntheticDatasetUseCase::new(generator)
            .execute(&blueprint)
            .expect("dataset");

        assert_eq!(report.dataset().trajectories().len(), 1);
    }

    fn synthetic_case(minimum_examples: usize) -> SyntheticCaseSpec {
        SyntheticCaseSpec::new(
            SyntheticCaseId::parse("case-1").expect("case"),
            OperatorMode::Read,
            TaskFamily::parse("read.inspect").expect("task"),
            KernelTool::Inspect,
            PositiveCount::parse(minimum_examples, "minimum_examples").expect("minimum"),
        )
        .expect("case spec")
    }

    fn trajectory_row(step_id: &str) -> serde_json::Value {
        json!({
            "step_id": step_id,
            "about": "about-1",
            "mode": "read",
            "task_family": "read.inspect",
            "allowed_tools": [
                "kernel_wake",
                "kernel_ask",
                "kernel_near",
                "kernel_goto",
                "kernel_rewind",
                "kernel_forward",
                "kernel_trace",
                "kernel_inspect"
            ],
            "visible_state": {},
            "target_action": {
                "type": "tool_call",
                "tool": "kernel_inspect",
                "arguments": { "ref": "node-1" }
            }
        })
    }

    fn trajectory() -> TrainingTrajectory {
        let mode = OperatorMode::Read;
        TrainingTrajectory::new(
            StepId::parse("step-1").expect("step"),
            AboutId::parse("about-1").expect("about"),
            mode,
            TaskFamily::parse("read.inspect").expect("task"),
            AllowedTools::all_for_mode(mode),
            VisibleState::parse(json!({})).expect("visible"),
            OperatorAction::tool_call(
                KernelTool::Inspect,
                ActionArguments::parse(json!({ "ref": "node-1" })).expect("arguments"),
            ),
        )
        .expect("trajectory")
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
            "underpass-operator-synthetic-infra must stay independent from kernel crates"
        );
    }
}
