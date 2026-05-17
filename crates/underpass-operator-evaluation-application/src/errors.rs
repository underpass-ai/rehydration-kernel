use std::error::Error;
use std::fmt::{Display, Formatter};

use underpass_operator_shared_domain::DomainError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadEvaluationCasesError {
    message: String,
}

impl ReadEvaluationCasesError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl Display for ReadEvaluationCasesError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for ReadEvaluationCasesError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvaluationApplicationError {
    ReadCases(ReadEvaluationCasesError),
    Domain(DomainError),
}

pub type EvaluationApplicationResult<T> = Result<T, EvaluationApplicationError>;

impl Display for EvaluationApplicationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReadCases(error) => write!(f, "{error}"),
            Self::Domain(error) => write!(f, "{error}"),
        }
    }
}

impl Error for EvaluationApplicationError {}

impl From<ReadEvaluationCasesError> for EvaluationApplicationError {
    fn from(value: ReadEvaluationCasesError) -> Self {
        Self::ReadCases(value)
    }
}

impl From<DomainError> for EvaluationApplicationError {
    fn from(value: DomainError) -> Self {
        Self::Domain(value)
    }
}
