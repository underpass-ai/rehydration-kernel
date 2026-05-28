use std::error::Error;
use std::fmt::{Display, Formatter};

use underpass_operator_shared_domain::DomainError;

#[derive(Debug)]
pub enum InfraError {
    Json(serde_json::Error),
    Domain(DomainError),
}

pub type InfraResult<T> = Result<T, InfraError>;

impl Display for InfraError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json(error) => write!(f, "{error}"),
            Self::Domain(error) => write!(f, "{error}"),
        }
    }
}

impl Error for InfraError {}

impl From<serde_json::Error> for InfraError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

impl From<DomainError> for InfraError {
    fn from(value: DomainError) -> Self {
        Self::Domain(value)
    }
}
