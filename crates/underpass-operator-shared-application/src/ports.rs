use underpass_operator_shared_domain::TrainingTrajectory;

use crate::ReadTrainingDatasetError;

pub trait TrainingTrajectoryReader {
    fn read_training_trajectories(
        &self,
    ) -> Result<Vec<TrainingTrajectory>, ReadTrainingDatasetError>;
}
