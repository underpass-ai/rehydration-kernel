use underpass_operator_training_domain::TrainingRunReadinessReport;

use crate::TrainingApplicationResult;
use crate::ports::TrainingRunPlanReader;

pub struct PrepareTrainingRunUseCase<R> {
    reader: R,
}

impl<R> PrepareTrainingRunUseCase<R>
where
    R: TrainingRunPlanReader,
{
    pub fn new(reader: R) -> Self {
        Self { reader }
    }

    pub fn execute(&self) -> TrainingApplicationResult<TrainingRunReadinessReport> {
        let plan = self.reader.read_training_run_plan()?;
        Ok(TrainingRunReadinessReport::new(plan))
    }
}
