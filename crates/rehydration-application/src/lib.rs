use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use rehydration_domain::{BundleMetadata, CaseId, DomainError, RehydrationBundle, Role};
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
    Validation(String),
}

impl fmt::Display for ApplicationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Domain(error) => error.fmt(f),
            Self::Ports(error) => error.fmt(f),
            Self::Validation(message) => f.write_str(message),
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
pub struct GetContextQuery {
    pub case_id: String,
    pub role: String,
    pub requested_scopes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetContextResult {
    pub bundle: RehydrationBundle,
    pub rendered: RenderedContext,
    pub scope_validation: ScopeValidation,
    pub served_at: SystemTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RehydrateSessionQuery {
    pub case_id: String,
    pub roles: Vec<String>,
    pub persist_snapshot: bool,
    pub timeline_window: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RehydrateSessionResult {
    pub case_id: String,
    pub bundles: Vec<RehydrationBundle>,
    pub timeline_events: u32,
    pub version: BundleMetadata,
    pub snapshot_persisted: bool,
    pub snapshot_id: Option<String>,
    pub generated_at: SystemTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidateScopeQuery {
    pub required_scopes: Vec<String>,
    pub provided_scopes: Vec<String>,
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
    pub snapshot_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetProjectionStatusQuery {
    pub consumer_names: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionStatusView {
    pub consumer_name: String,
    pub stream_name: String,
    pub projection_watermark: String,
    pub processed_events: u64,
    pub pending_events: u64,
    pub last_event_at: SystemTime,
    pub updated_at: SystemTime,
    pub healthy: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetProjectionStatusResult {
    pub projections: Vec<ProjectionStatusView>,
    pub observed_at: SystemTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayModeSelection {
    DryRun,
    Rebuild,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayProjectionCommand {
    pub consumer_name: String,
    pub stream_name: String,
    pub starting_after: Option<String>,
    pub max_events: u32,
    pub replay_mode: ReplayModeSelection,
    pub requested_by: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayProjectionOutcome {
    pub replay_id: String,
    pub consumer_name: String,
    pub replay_mode: ReplayModeSelection,
    pub accepted_events: u32,
    pub requested_at: SystemTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetBundleSnapshotQuery {
    pub case_id: String,
    pub role: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleSnapshotResult {
    pub snapshot_id: String,
    pub case_id: String,
    pub role: String,
    pub bundle: RehydrationBundle,
    pub created_at: SystemTime,
    pub expires_at: SystemTime,
    pub ttl_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetGraphRelationshipsQuery {
    pub node_id: String,
    pub node_kind: Option<String>,
    pub depth: u32,
    pub include_reverse_edges: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphNodeView {
    pub node_id: String,
    pub node_kind: String,
    pub title: String,
    pub labels: Vec<String>,
    pub properties: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphRelationshipView {
    pub source_node_id: String,
    pub target_node_id: String,
    pub relationship_type: String,
    pub properties: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetGraphRelationshipsResult {
    pub root: GraphNodeView,
    pub neighbors: Vec<GraphNodeView>,
    pub relationships: Vec<GraphRelationshipView>,
    pub observed_at: SystemTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetRehydrationDiagnosticsQuery {
    pub case_id: String,
    pub roles: Vec<String>,
    pub phase: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RehydrationDiagnosticView {
    pub role: String,
    pub version: BundleMetadata,
    pub selected_decisions: u32,
    pub selected_impacts: u32,
    pub selected_milestones: u32,
    pub estimated_tokens: u32,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetRehydrationDiagnosticsResult {
    pub diagnostics: Vec<RehydrationDiagnosticView>,
    pub observed_at: SystemTime,
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

    pub async fn execute(
        &self,
        case_id: &str,
        role: &str,
    ) -> Result<RehydrationBundle, ApplicationError> {
        let case_id = CaseId::new(case_id)?;
        let role = Role::new(role)?;

        let bundle = match self.projection_reader.load_bundle(&case_id, &role).await? {
            Some(bundle) => bundle,
            None => RehydrationBundle::empty(case_id, role, self.generator_version),
        };

        self.snapshot_store.save_bundle(&bundle).await?;
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

    pub async fn execute(
        &self,
        case_id: &str,
        role: &str,
        requested_scopes: &[String],
    ) -> Result<GetContextResult, ApplicationError> {
        let bundle = self.rehydrate_session.execute(case_id, role).await?;
        let rendered = render_bundle(&bundle);
        let scope_validation = ValidateScopeUseCase::execute(requested_scopes, requested_scopes);

        Ok(GetContextResult {
            bundle,
            rendered,
            scope_validation,
            served_at: SystemTime::now(),
        })
    }
}

#[derive(Debug)]
pub struct QueryApplicationService<R, S> {
    projection_reader: Arc<R>,
    snapshot_store: Arc<S>,
    generator_version: &'static str,
}

impl<R, S> QueryApplicationService<R, S>
where
    R: ProjectionReader + Send + Sync,
    S: SnapshotStore + Send + Sync,
{
    pub fn new(
        projection_reader: Arc<R>,
        snapshot_store: Arc<S>,
        generator_version: &'static str,
    ) -> Self {
        Self {
            projection_reader,
            snapshot_store,
            generator_version,
        }
    }

    pub async fn get_context(
        &self,
        query: GetContextQuery,
    ) -> Result<GetContextResult, ApplicationError> {
        let rehydrate = RehydrateSessionUseCase::new(
            Arc::clone(&self.projection_reader),
            Arc::clone(&self.snapshot_store),
            self.generator_version,
        );

        GetContextUseCase::new(rehydrate)
            .execute(&query.case_id, &query.role, &query.requested_scopes)
            .await
    }

    pub async fn rehydrate_session(
        &self,
        query: RehydrateSessionQuery,
    ) -> Result<RehydrateSessionResult, ApplicationError> {
        if query.roles.is_empty() {
            return Err(ApplicationError::Validation(
                "roles cannot be empty".to_string(),
            ));
        }

        let mut bundles = Vec::with_capacity(query.roles.len());
        for role in &query.roles {
            bundles.push(self.rehydrate_single(&query.case_id, role).await?);
        }

        let snapshot_id = if query.persist_snapshot {
            Some(format!(
                "snapshot:{}:{}",
                query.case_id,
                query.roles.join(",")
            ))
        } else {
            None
        };

        Ok(RehydrateSessionResult {
            case_id: query.case_id,
            bundles,
            timeline_events: query.timeline_window,
            version: BundleMetadata::initial(self.generator_version),
            snapshot_persisted: query.persist_snapshot,
            snapshot_id,
            generated_at: SystemTime::now(),
        })
    }

    pub fn validate_scope(&self, query: ValidateScopeQuery) -> ScopeValidation {
        ValidateScopeUseCase::execute(&query.required_scopes, &query.provided_scopes)
    }

    pub async fn warmup_bundle(&self) -> Result<RehydrationBundle, ApplicationError> {
        self.rehydrate_single("bootstrap-case", "system").await
    }

    async fn rehydrate_single(
        &self,
        case_id: &str,
        role: &str,
    ) -> Result<RehydrationBundle, ApplicationError> {
        RehydrateSessionUseCase::new(
            Arc::clone(&self.projection_reader),
            Arc::clone(&self.snapshot_store),
            self.generator_version,
        )
        .execute(case_id, role)
        .await
    }
}

#[derive(Debug)]
pub struct AdminApplicationService<R> {
    projection_reader: Arc<R>,
    generator_version: &'static str,
}

impl<R> AdminApplicationService<R>
where
    R: ProjectionReader + Send + Sync,
{
    pub fn new(projection_reader: Arc<R>, generator_version: &'static str) -> Self {
        Self {
            projection_reader,
            generator_version,
        }
    }

    pub fn get_projection_status(
        &self,
        query: GetProjectionStatusQuery,
    ) -> GetProjectionStatusResult {
        let observed_at = SystemTime::now();
        let consumer_names = if query.consumer_names.is_empty() {
            vec!["context-projection".to_string()]
        } else {
            query.consumer_names
        };

        GetProjectionStatusResult {
            projections: consumer_names
                .into_iter()
                .map(|consumer_name| ProjectionStatusView {
                    stream_name: format!("{consumer_name}.events"),
                    consumer_name,
                    projection_watermark: "rev-0".to_string(),
                    processed_events: 0,
                    pending_events: 0,
                    last_event_at: observed_at,
                    updated_at: observed_at,
                    healthy: true,
                    warnings: vec!["projection status is placeholder-backed".to_string()],
                })
                .collect(),
            observed_at,
        }
    }

    pub fn replay_projection(
        &self,
        command: ReplayProjectionCommand,
    ) -> Result<ReplayProjectionOutcome, ApplicationError> {
        let consumer_name = require_non_empty(command.consumer_name, "consumer_name")?;
        let stream_name = require_non_empty(command.stream_name, "stream_name")?;

        Ok(ReplayProjectionOutcome {
            replay_id: format!("replay:{consumer_name}:{stream_name}"),
            consumer_name,
            replay_mode: command.replay_mode,
            accepted_events: command.max_events,
            requested_at: SystemTime::now(),
        })
    }

    pub async fn get_bundle_snapshot(
        &self,
        query: GetBundleSnapshotQuery,
    ) -> Result<BundleSnapshotResult, ApplicationError> {
        let bundle = self
            .load_or_empty_bundle(&query.case_id, &query.role)
            .await?;
        let created_at = SystemTime::now();
        let ttl_seconds = 900;
        let expires_at = created_at
            .checked_add(Duration::from_secs(ttl_seconds))
            .unwrap_or(created_at);

        Ok(BundleSnapshotResult {
            snapshot_id: format!(
                "snapshot:{}:{}",
                bundle.case_id().as_str(),
                bundle.role().as_str()
            ),
            case_id: bundle.case_id().as_str().to_string(),
            role: bundle.role().as_str().to_string(),
            bundle,
            created_at,
            expires_at,
            ttl_seconds,
        })
    }

    pub fn get_graph_relationships(
        &self,
        query: GetGraphRelationshipsQuery,
    ) -> Result<GetGraphRelationshipsResult, ApplicationError> {
        let node_id = require_non_empty(query.node_id, "node_id")?;
        let node_kind = query
            .node_kind
            .and_then(|value| trim_to_option(value.as_str()))
            .unwrap_or_else(|| "unknown".to_string());

        let root = GraphNodeView {
            node_id: node_id.clone(),
            node_kind: node_kind.clone(),
            title: format!("{} {}", node_kind, node_id),
            labels: vec![node_kind.clone()],
            properties: BTreeMap::from([
                ("depth".to_string(), query.depth.to_string()),
                ("source".to_string(), "admin-placeholder".to_string()),
            ]),
        };

        let mut neighbors = Vec::new();
        let mut relationships = Vec::new();

        if query.depth > 0 {
            let child_id = format!("{node_id}-neighbor-1");
            neighbors.push(GraphNodeView {
                node_id: child_id.clone(),
                node_kind: node_kind.clone(),
                title: format!("Related {child_id}"),
                labels: vec!["related".to_string()],
                properties: BTreeMap::from([(
                    "edge_direction".to_string(),
                    "outbound".to_string(),
                )]),
            });
            relationships.push(GraphRelationshipView {
                source_node_id: node_id.clone(),
                target_node_id: child_id,
                relationship_type: "RELATES_TO".to_string(),
                properties: BTreeMap::new(),
            });
        }

        if query.include_reverse_edges {
            let reverse_id = format!("{node_id}-neighbor-reverse");
            neighbors.push(GraphNodeView {
                node_id: reverse_id.clone(),
                node_kind,
                title: format!("Reverse {reverse_id}"),
                labels: vec!["reverse".to_string()],
                properties: BTreeMap::from([("edge_direction".to_string(), "inbound".to_string())]),
            });
            relationships.push(GraphRelationshipView {
                source_node_id: reverse_id,
                target_node_id: node_id,
                relationship_type: "INFLUENCES".to_string(),
                properties: BTreeMap::new(),
            });
        }

        Ok(GetGraphRelationshipsResult {
            root,
            neighbors,
            relationships,
            observed_at: SystemTime::now(),
        })
    }

    pub async fn get_rehydration_diagnostics(
        &self,
        query: GetRehydrationDiagnosticsQuery,
    ) -> Result<GetRehydrationDiagnosticsResult, ApplicationError> {
        if query.roles.is_empty() {
            return Err(ApplicationError::Validation(
                "roles cannot be empty".to_string(),
            ));
        }

        let phase = query
            .phase
            .and_then(|value| trim_to_option(value.as_str()))
            .unwrap_or_else(|| "PHASE_UNSPECIFIED".to_string());
        let observed_at = SystemTime::now();
        let diagnostics = query
            .roles
            .iter()
            .map(|role| async {
                self.load_or_empty_bundle(&query.case_id, role)
                    .await
                    .map(|bundle| RehydrationDiagnosticView {
                        role: role.clone(),
                        version: bundle.metadata().clone(),
                        selected_decisions: 0,
                        selected_impacts: 0,
                        selected_milestones: 0,
                        estimated_tokens: bundle
                            .sections()
                            .iter()
                            .map(|section| section.split_whitespace().count() as u32)
                            .sum(),
                        notes: vec![format!("phase={phase}")],
                    })
            })
            .collect::<Vec<_>>();
        let mut collected = Vec::with_capacity(diagnostics.len());
        for diagnostic in diagnostics {
            collected.push(diagnostic.await?);
        }

        Ok(GetRehydrationDiagnosticsResult {
            diagnostics: collected,
            observed_at,
        })
    }

    async fn load_or_empty_bundle(
        &self,
        case_id: &str,
        role: &str,
    ) -> Result<RehydrationBundle, ApplicationError> {
        let case_id = CaseId::new(case_id)?;
        let role = Role::new(role)?;

        match self.projection_reader.load_bundle(&case_id, &role).await? {
            Some(bundle) => Ok(bundle),
            None => Ok(RehydrationBundle::empty(
                case_id,
                role,
                self.generator_version,
            )),
        }
    }
}

#[derive(Debug)]
pub struct CommandApplicationService {
    update_context: Arc<UpdateContextUseCase>,
}

impl CommandApplicationService {
    pub fn new(update_context: Arc<UpdateContextUseCase>) -> Self {
        Self { update_context }
    }

    pub fn update_context(
        &self,
        command: UpdateContextCommand,
    ) -> Result<UpdateContextOutcome, ApplicationError> {
        let snapshot_id = if command.persist_snapshot {
            Some(format!("snapshot:{}:{}", command.case_id, command.role))
        } else {
            None
        };

        let mut outcome = self.update_context.execute(command)?;
        outcome.snapshot_id = snapshot_id;
        Ok(outcome)
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
            snapshot_id: None,
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

fn require_non_empty(value: String, field: &'static str) -> Result<String, ApplicationError> {
    trim_to_option(&value)
        .ok_or_else(|| ApplicationError::Validation(format!("{field} cannot be empty")))
}

fn trim_to_option(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::SystemTime;

    use rehydration_domain::{CaseId, RehydrationBundle, Role};
    use rehydration_ports::{PortError, ProjectionReader, SnapshotStore};

    use super::{
        AdminApplicationService, CommandApplicationService, GetBundleSnapshotQuery,
        GetContextQuery, GetContextUseCase, GetProjectionStatusQuery,
        GetRehydrationDiagnosticsQuery, QueryApplicationService, RehydrateSessionQuery,
        RehydrateSessionUseCase, ReplayModeSelection, ReplayProjectionCommand,
        UpdateContextCommand, UpdateContextUseCase, ValidateScopeQuery, ValidateScopeUseCase,
        dedupe_scopes, render_bundle, require_non_empty, trim_to_option,
    };

    struct EmptyProjectionReader;

    impl ProjectionReader for EmptyProjectionReader {
        async fn load_bundle(
            &self,
            _case_id: &CaseId,
            _role: &Role,
        ) -> Result<Option<RehydrationBundle>, PortError> {
            Ok(None)
        }
    }

    struct RecordingSnapshotStore;

    impl SnapshotStore for RecordingSnapshotStore {
        async fn save_bundle(&self, _bundle: &RehydrationBundle) -> Result<(), PortError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn use_case_builds_a_placeholder_bundle_when_projection_is_empty() {
        let use_case =
            RehydrateSessionUseCase::new(EmptyProjectionReader, RecordingSnapshotStore, "0.1.0");

        let bundle = use_case
            .execute("case-123", "system")
            .await
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

    #[tokio::test]
    async fn get_context_renders_placeholder_content() {
        let rehydrate =
            RehydrateSessionUseCase::new(EmptyProjectionReader, RecordingSnapshotStore, "0.1.0");
        let use_case = GetContextUseCase::new(rehydrate);

        let result = use_case
            .execute("case-123", "system", &["decisions".to_string()])
            .await
            .expect("get context should succeed");

        assert!(result.rendered.content.contains("bundle for case case-123"));
        assert!(result.scope_validation.allowed);
    }

    #[tokio::test]
    async fn query_application_service_rehydrates_multiple_roles() {
        let service = QueryApplicationService::new(
            Arc::new(EmptyProjectionReader),
            Arc::new(RecordingSnapshotStore),
            "0.1.0",
        );

        let result = service
            .rehydrate_session(RehydrateSessionQuery {
                case_id: "case-123".to_string(),
                roles: vec!["developer".to_string(), "reviewer".to_string()],
                persist_snapshot: true,
                timeline_window: 32,
            })
            .await
            .expect("rehydration should succeed");

        assert_eq!(result.bundles.len(), 2);
        assert!(result.snapshot_persisted);
        assert_eq!(result.timeline_events, 32);
    }

    #[test]
    fn query_application_service_validates_scope_queries() {
        let service = QueryApplicationService::new(
            Arc::new(EmptyProjectionReader),
            Arc::new(RecordingSnapshotStore),
            "0.1.0",
        );

        let result = service.validate_scope(ValidateScopeQuery {
            required_scopes: vec!["decisions".to_string()],
            provided_scopes: vec!["milestones".to_string()],
        });

        assert!(!result.allowed);
        assert_eq!(result.missing_scopes, vec!["decisions".to_string()]);
    }

    #[test]
    fn admin_application_service_uses_default_projection_consumer() {
        let service = AdminApplicationService::new(Arc::new(EmptyProjectionReader), "0.1.0");

        let result = service.get_projection_status(GetProjectionStatusQuery {
            consumer_names: Vec::new(),
        });

        assert_eq!(result.projections.len(), 1);
        assert_eq!(result.projections[0].consumer_name, "context-projection");
    }

    #[tokio::test]
    async fn admin_application_service_builds_snapshot_and_replay() {
        let service = AdminApplicationService::new(Arc::new(EmptyProjectionReader), "0.1.0");

        let snapshot = service
            .get_bundle_snapshot(GetBundleSnapshotQuery {
                case_id: "case-123".to_string(),
                role: "developer".to_string(),
            })
            .await
            .expect("snapshot should succeed");
        assert_eq!(snapshot.case_id, "case-123");
        assert_eq!(snapshot.ttl_seconds, 900);

        let replay = service
            .replay_projection(ReplayProjectionCommand {
                consumer_name: "context-projection".to_string(),
                stream_name: "planning.story.created".to_string(),
                starting_after: Some("evt-42".to_string()),
                max_events: 50,
                replay_mode: ReplayModeSelection::DryRun,
                requested_by: Some("operator".to_string()),
            })
            .expect("replay should succeed");
        assert_eq!(replay.consumer_name, "context-projection");
        assert_eq!(replay.accepted_events, 50);
    }

    #[tokio::test]
    async fn admin_application_service_requires_roles_for_diagnostics() {
        let service = AdminApplicationService::new(Arc::new(EmptyProjectionReader), "0.1.0");

        let error = service
            .get_rehydration_diagnostics(GetRehydrationDiagnosticsQuery {
                case_id: "case-123".to_string(),
                roles: Vec::new(),
                phase: Some("PHASE_BUILD".to_string()),
            })
            .await
            .expect_err("diagnostics should require at least one role");

        assert_eq!(error.to_string(), "roles cannot be empty");
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
        assert!(result.snapshot_id.is_none());
        assert_eq!(result.warnings.len(), 3);
    }

    #[tokio::test]
    async fn get_context_query_round_trip_works() {
        let service = QueryApplicationService::new(
            Arc::new(EmptyProjectionReader),
            Arc::new(RecordingSnapshotStore),
            "0.1.0",
        );

        let result = service
            .get_context(GetContextQuery {
                case_id: "case-123".to_string(),
                role: "developer".to_string(),
                requested_scopes: vec!["decisions".to_string()],
            })
            .await
            .expect("get context should succeed");

        assert_eq!(result.bundle.case_id().as_str(), "case-123");
        assert!(result.served_at <= SystemTime::now());
    }

    #[test]
    fn command_application_service_adds_snapshot_metadata() {
        let service = CommandApplicationService::new(Arc::new(UpdateContextUseCase::new("0.1.0")));

        let result = service
            .update_context(UpdateContextCommand {
                case_id: "case-123".to_string(),
                role: "developer".to_string(),
                work_item_id: "task-7".to_string(),
                changes: Vec::new(),
                expected_revision: Some(1),
                expected_content_hash: None,
                idempotency_key: None,
                requested_by: Some("agent".to_string()),
                persist_snapshot: true,
            })
            .expect("command service should succeed");

        assert_eq!(
            result.snapshot_id.as_deref(),
            Some("snapshot:case-123:developer")
        );
        assert!(result.snapshot_persisted);
    }

    #[test]
    fn helper_functions_trim_render_and_validate() {
        let bundle = RehydrationBundle::new(
            CaseId::new("case-123").expect("case id is valid"),
            Role::new("developer").expect("role is valid"),
            vec!["first section".to_string(), "second section".to_string()],
            rehydration_domain::BundleMetadata::initial("0.1.0"),
        );
        let rendered = render_bundle(&bundle);

        assert_eq!(rendered.sections.len(), 2);
        assert_eq!(rendered.token_count, 4);
        assert_eq!(
            dedupe_scopes(&[
                " decisions ".to_string(),
                "".to_string(),
                "tasks".to_string(),
                "decisions".to_string(),
            ]),
            vec!["decisions".to_string(), "tasks".to_string()]
        );
        assert_eq!(
            require_non_empty("  case-123  ".to_string(), "case_id")
                .expect("value should be trimmed"),
            "case-123"
        );
        assert_eq!(trim_to_option("  scoped  "), Some("scoped".to_string()));
        assert_eq!(trim_to_option("   "), None);
        let error =
            require_non_empty("   ".to_string(), "case_id").expect_err("empty values must fail");
        assert_eq!(error.to_string(), "case_id cannot be empty");
    }

    #[test]
    fn validate_scope_passes_when_inputs_are_equivalent_after_trimming() {
        let result = ValidateScopeUseCase::execute(
            &[
                " decisions ".to_string(),
                "tasks".to_string(),
                "tasks".to_string(),
            ],
            &["tasks".to_string(), "decisions".to_string()],
        );

        assert!(result.allowed);
        assert_eq!(result.reason, "scope validation passed");
        assert!(result.missing_scopes.is_empty());
        assert!(result.extra_scopes.is_empty());
    }
}
