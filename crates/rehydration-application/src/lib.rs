use std::error::Error;
use std::fmt;

use rehydration_domain::{CaseId, DomainError, RehydrationBundle, Role};
use rehydration_ports::{PortError, ProjectionReader, SnapshotStore};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RehydrationApplication;

impl RehydrationApplication {
    pub const fn capability_name() -> &'static str {
        "deterministic-context-rehydration"
    }
}

#[derive(Debug)]
pub enum ApplicationError {
    Domain(DomainError),
    Ports(PortError),
}

impl fmt::Display for ApplicationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Domain(error) => error.fmt(f),
            Self::Ports(error) => error.fmt(f),
        }
    }
}

impl Error for ApplicationError {}

impl From<DomainError> for ApplicationError {
    fn from(value: DomainError) -> Self {
        Self::Domain(value)
    }
}

impl From<PortError> for ApplicationError {
    fn from(value: PortError) -> Self {
        Self::Ports(value)
    }
}

#[derive(Debug)]
pub struct RehydrateSessionUseCase<R, S> {
    projection_reader: R,
    snapshot_store: S,
    generator_version: &'static str,
}

impl<R, S> RehydrateSessionUseCase<R, S>
where
    R: ProjectionReader,
    S: SnapshotStore,
{
    pub fn new(projection_reader: R, snapshot_store: S, generator_version: &'static str) -> Self {
        Self {
            projection_reader,
            snapshot_store,
            generator_version,
        }
    }

    pub fn execute(
        &self,
        case_id: &str,
        role: &str,
    ) -> Result<RehydrationBundle, ApplicationError> {
        let case_id = CaseId::new(case_id)?;
        let role = Role::new(role)?;

        let bundle = match self.projection_reader.load_bundle(&case_id, &role)? {
            Some(bundle) => bundle,
            None => RehydrationBundle::empty(case_id, role, self.generator_version),
        };

        self.snapshot_store.save_bundle(&bundle)?;
        Ok(bundle)
    }
}

#[cfg(test)]
mod tests {
    use rehydration_domain::{CaseId, RehydrationBundle, Role};
    use rehydration_ports::{PortError, ProjectionReader, SnapshotStore};

    use super::RehydrateSessionUseCase;

    struct EmptyProjectionReader;

    impl ProjectionReader for EmptyProjectionReader {
        fn load_bundle(
            &self,
            _case_id: &CaseId,
            _role: &Role,
        ) -> Result<Option<RehydrationBundle>, PortError> {
            Ok(None)
        }
    }

    struct RecordingSnapshotStore;

    impl SnapshotStore for RecordingSnapshotStore {
        fn save_bundle(&self, _bundle: &RehydrationBundle) -> Result<(), PortError> {
            Ok(())
        }
    }

    #[test]
    fn use_case_builds_a_placeholder_bundle_when_projection_is_empty() {
        let use_case =
            RehydrateSessionUseCase::new(EmptyProjectionReader, RecordingSnapshotStore, "0.1.0");

        let bundle = use_case
            .execute("case-123", "system")
            .expect("placeholder bundle should be built");

        assert_eq!(bundle.case_id().as_str(), "case-123");
        assert_eq!(bundle.role().as_str(), "system");
    }
}
