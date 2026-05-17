use underpass_operator_shared_domain::TrainingTrajectory;
use underpass_operator_training_domain::TrainingRunPlan;

use crate::ReadTrainingRunPlanError;

pub trait TrainingRunPlanReader {
    fn read_training_run_plan(&self) -> Result<TrainingRunPlan, ReadTrainingRunPlanError>;
}

pub trait TrainingTrajectoryReader {
    fn read_training_trajectories(
        &self,
    ) -> Result<Vec<TrainingTrajectory>, ReadTrainingRunPlanError>;
}
