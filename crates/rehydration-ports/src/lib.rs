use std::error::Error;
use std::fmt;

use rehydration_domain::{CaseId, RehydrationBundle, Role};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortError {
    InvalidState(String),
    Unavailable(String),
}

impl fmt::Display for PortError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidState(message) | Self::Unavailable(message) => f.write_str(message),
        }
    }
}

impl Error for PortError {}

pub trait ProjectionReader {
    fn load_bundle(
        &self,
        case_id: &CaseId,
        role: &Role,
    ) -> Result<Option<RehydrationBundle>, PortError>;
}

pub trait SnapshotStore {
    fn save_bundle(&self, bundle: &RehydrationBundle) -> Result<(), PortError>;
}

#[cfg(test)]
mod tests {
    use super::PortError;

    #[test]
    fn port_error_uses_inner_message() {
        let error = PortError::Unavailable("neo4j unavailable".to_string());
        assert_eq!(error.to_string(), "neo4j unavailable");
    }
}
