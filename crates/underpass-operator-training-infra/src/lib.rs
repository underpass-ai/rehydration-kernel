use underpass_operator_shared_domain::TrainingTrajectory;
use underpass_operator_training_application::{
    ReadTrainingRunPlanError,
    ports::{TrainingRunPlanReader, TrainingTrajectoryReader},
};
use underpass_operator_training_domain::TrainingRunPlan;

pub struct InMemoryTrainingRunPlanReader {
    plan: TrainingRunPlan,
}

impl InMemoryTrainingRunPlanReader {
    pub fn new(plan: TrainingRunPlan) -> Self {
        Self { plan }
    }
}

impl TrainingRunPlanReader for InMemoryTrainingRunPlanReader {
    fn read_training_run_plan(&self) -> Result<TrainingRunPlan, ReadTrainingRunPlanError> {
        Ok(self.plan.clone())
    }
}

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
    ) -> Result<Vec<TrainingTrajectory>, ReadTrainingRunPlanError> {
        Ok(self.trajectories.clone())
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
            "underpass-operator-training-infra must stay independent from kernel crates"
        );
    }
}
