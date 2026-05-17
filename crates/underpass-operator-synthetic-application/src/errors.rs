use std::error::Error;
use std::fmt::{Display, Formatter};

use underpass_operator_shared_domain::DomainError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerateSyntheticCaseError {
    message: String,
}

impl GenerateSyntheticCaseError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl Display for GenerateSyntheticCaseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for GenerateSyntheticCaseError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyntheticApplicationError {
    GenerateCase(GenerateSyntheticCaseError),
    Domain(DomainError),
}

pub type SyntheticApplicationResult<T> = Result<T, SyntheticApplicationError>;

impl Display for SyntheticApplicationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GenerateCase(error) => write!(f, "{error}"),
            Self::Domain(error) => write!(f, "{error}"),
        }
    }
}

impl Error for SyntheticApplicationError {}

impl From<GenerateSyntheticCaseError> for SyntheticApplicationError {
    fn from(value: GenerateSyntheticCaseError) -> Self {
        Self::GenerateCase(value)
    }
}

impl From<DomainError> for SyntheticApplicationError {
    fn from(value: DomainError) -> Self {
        Self::Domain(value)
    }
}
