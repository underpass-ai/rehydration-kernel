use underpass_operator_shared_domain::TrainingTrajectory;
use underpass_operator_synthetic_domain::SyntheticCaseSpec;

use crate::GenerateSyntheticCaseError;

pub trait SyntheticCaseGenerator {
    fn generate_case(
        &self,
        spec: &SyntheticCaseSpec,
    ) -> Result<Vec<TrainingTrajectory>, GenerateSyntheticCaseError>;
}
