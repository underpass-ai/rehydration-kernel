use std::error::Error;
use std::fmt::{Display, Formatter};

use underpass_operator_shared_domain::DomainError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadTrainingRunPlanError {
    message: String,
}

impl ReadTrainingRunPlanError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl Display for ReadTrainingRunPlanError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for ReadTrainingRunPlanError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrainingApplicationError {
    ReadPlan(ReadTrainingRunPlanError),
    ReadTrainingTrajectories(ReadTrainingRunPlanError),
    Domain(DomainError),
}

pub type TrainingApplicationResult<T> = Result<T, TrainingApplicationError>;

impl Display for TrainingApplicationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReadPlan(error) => write!(f, "{error}"),
            Self::ReadTrainingTrajectories(error) => write!(f, "{error}"),
            Self::Domain(error) => write!(f, "{error}"),
        }
    }
}

impl Error for TrainingApplicationError {}

impl From<ReadTrainingRunPlanError> for TrainingApplicationError {
    fn from(value: ReadTrainingRunPlanError) -> Self {
        Self::ReadPlan(value)
    }
}

impl From<DomainError> for TrainingApplicationError {
    fn from(value: DomainError) -> Self {
        Self::Domain(value)
    }
}
