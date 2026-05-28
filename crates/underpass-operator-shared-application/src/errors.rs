use std::error::Error;
use std::fmt::{Display, Formatter};

use underpass_operator_shared_domain::DomainError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadTrainingDatasetError {
    message: String,
}

impl ReadTrainingDatasetError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl Display for ReadTrainingDatasetError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for ReadTrainingDatasetError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplicationError {
    ReadTrainingDataset(ReadTrainingDatasetError),
    Domain(DomainError),
}

pub type ApplicationResult<T> = Result<T, ApplicationError>;

impl Display for ApplicationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReadTrainingDataset(error) => write!(f, "{error}"),
            Self::Domain(error) => write!(f, "{error}"),
        }
    }
}

impl Error for ApplicationError {}

impl From<ReadTrainingDatasetError> for ApplicationError {
    fn from(value: ReadTrainingDatasetError) -> Self {
        Self::ReadTrainingDataset(value)
    }
}

impl From<DomainError> for ApplicationError {
    fn from(value: DomainError) -> Self {
        Self::Domain(value)
    }
}
