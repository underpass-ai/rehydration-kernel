use std::collections::BTreeSet;
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeValidation {
    pub allowed: bool,
    pub required_scopes: Vec<String>,
    pub provided_scopes: Vec<String>,
    pub missing_scopes: Vec<String>,
    pub extra_scopes: Vec<String>,
    pub reason: String,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedContext {
    pub content: String,
    pub token_count: u32,
    pub sections: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetContextResult {
    pub bundle: RehydrationBundle,
    pub rendered: RenderedContext,
    pub scope_validation: ScopeValidation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateContextChange {
    pub operation: String,
    pub entity_kind: String,
    pub entity_id: String,
    pub payload_json: String,
    pub reason: String,
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateContextCommand {
    pub case_id: String,
    pub role: String,
    pub work_item_id: String,
    pub changes: Vec<UpdateContextChange>,
    pub expected_revision: Option<u64>,
    pub expected_content_hash: Option<String>,
    pub idempotency_key: Option<String>,
    pub requested_by: Option<String>,
    pub persist_snapshot: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedVersion {
    pub revision: u64,
    pub content_hash: String,
    pub generator_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateContextOutcome {
    pub accepted_version: AcceptedVersion,
    pub warnings: Vec<String>,
    pub snapshot_persisted: bool,
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

#[derive(Debug)]
pub struct ValidateScopeUseCase;

impl ValidateScopeUseCase {
    pub fn execute(required_scopes: &[String], provided_scopes: &[String]) -> ScopeValidation {
        let required = dedupe_scopes(required_scopes);
        let provided = dedupe_scopes(provided_scopes);

        let required_set: BTreeSet<_> = required.iter().cloned().collect();
        let provided_set: BTreeSet<_> = provided.iter().cloned().collect();

        let missing_scopes = required_set
            .difference(&provided_set)
            .cloned()
            .collect::<Vec<_>>();
        let extra_scopes = provided_set
            .difference(&required_set)
            .cloned()
            .collect::<Vec<_>>();
        let allowed = missing_scopes.is_empty() && extra_scopes.is_empty();
        let reason = if allowed {
            "scope validation passed".to_string()
        } else {
            "scope validation failed".to_string()
        };

        ScopeValidation {
            allowed,
            required_scopes: required,
            provided_scopes: provided,
            missing_scopes,
            extra_scopes,
            reason,
            diagnostics: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct GetContextUseCase<R, S> {
    rehydrate_session: RehydrateSessionUseCase<R, S>,
}

impl<R, S> GetContextUseCase<R, S>
where
    R: ProjectionReader,
    S: SnapshotStore,
{
    pub fn new(rehydrate_session: RehydrateSessionUseCase<R, S>) -> Self {
        Self { rehydrate_session }
    }

    pub fn execute(
        &self,
        case_id: &str,
        role: &str,
        requested_scopes: &[String],
    ) -> Result<GetContextResult, ApplicationError> {
        let bundle = self.rehydrate_session.execute(case_id, role)?;
        let rendered = render_bundle(&bundle);
        let scope_validation = ValidateScopeUseCase::execute(requested_scopes, requested_scopes);

        Ok(GetContextResult {
            bundle,
            rendered,
            scope_validation,
        })
    }
}

#[derive(Debug)]
pub struct UpdateContextUseCase {
    generator_version: &'static str,
}

impl UpdateContextUseCase {
    pub fn new(generator_version: &'static str) -> Self {
        Self { generator_version }
    }

    pub fn execute(
        &self,
        command: UpdateContextCommand,
    ) -> Result<UpdateContextOutcome, ApplicationError> {
        let case_id = CaseId::new(command.case_id)?;
        let role = Role::new(command.role)?;

        let revision = command.expected_revision.unwrap_or(0) + 1;
        let content_hash = format!(
            "{}:{}:{}:{}",
            case_id.as_str(),
            role.as_str(),
            command.work_item_id,
            command.changes.len()
        );

        let mut warnings = Vec::new();
        if command.changes.is_empty() {
            warnings.push("no changes supplied; update was accepted as a no-op".to_string());
        }
        if command.expected_content_hash.is_none() {
            warnings.push(
                "expected_content_hash missing; optimistic verification is partial".to_string(),
            );
        }
        if command.idempotency_key.is_none() {
            warnings.push(
                "idempotency_key missing; duplicate suppression is delegated upstream".to_string(),
            );
        }

        Ok(UpdateContextOutcome {
            accepted_version: AcceptedVersion {
                revision,
                content_hash,
                generator_version: self.generator_version.to_string(),
            },
            warnings,
            snapshot_persisted: command.persist_snapshot,
        })
    }
}

fn render_bundle(bundle: &RehydrationBundle) -> RenderedContext {
    let sections = if bundle.sections().is_empty() {
        vec![format!(
            "bundle for case {} role {}",
            bundle.case_id().as_str(),
            bundle.role().as_str()
        )]
    } else {
        bundle.sections().to_vec()
    };
    let content = sections.join("\n\n");
    let token_count = content.split_whitespace().count() as u32;

    RenderedContext {
        content,
        token_count,
        sections,
    }
}

fn dedupe_scopes(scopes: &[String]) -> Vec<String> {
    scopes
        .iter()
        .map(|scope| scope.trim())
        .filter(|scope| !scope.is_empty())
        .map(ToOwned::to_owned)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    use rehydration_domain::{CaseId, RehydrationBundle, Role};
    use rehydration_ports::{PortError, ProjectionReader, SnapshotStore};

    use super::{
        GetContextUseCase, RehydrateSessionUseCase, UpdateContextCommand, UpdateContextUseCase,
        ValidateScopeUseCase,
    };

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

    #[test]
    fn validate_scope_detects_missing_and_extra_scopes() {
        let result = ValidateScopeUseCase::execute(
            &["decisions".to_string(), "tasks".to_string()],
            &["decisions".to_string(), "milestones".to_string()],
        );

        assert!(!result.allowed);
        assert_eq!(result.missing_scopes, vec!["tasks".to_string()]);
        assert_eq!(result.extra_scopes, vec!["milestones".to_string()]);
    }

    #[test]
    fn get_context_renders_placeholder_content() {
        let rehydrate =
            RehydrateSessionUseCase::new(EmptyProjectionReader, RecordingSnapshotStore, "0.1.0");
        let use_case = GetContextUseCase::new(rehydrate);

        let result = use_case
            .execute("case-123", "system", &["decisions".to_string()])
            .expect("get context should succeed");

        assert!(result.rendered.content.contains("bundle for case case-123"));
        assert!(result.scope_validation.allowed);
    }

    #[test]
    fn update_context_builds_deterministic_version() {
        let use_case = UpdateContextUseCase::new("0.1.0");

        let result = use_case
            .execute(UpdateContextCommand {
                case_id: "case-123".to_string(),
                role: "developer".to_string(),
                work_item_id: "task-7".to_string(),
                changes: Vec::new(),
                expected_revision: Some(4),
                expected_content_hash: None,
                idempotency_key: None,
                requested_by: Some("agent".to_string()),
                persist_snapshot: true,
            })
            .expect("update should succeed");

        assert_eq!(result.accepted_version.revision, 5);
        assert!(result.snapshot_persisted);
        assert_eq!(result.warnings.len(), 3);
    }
}
