use std::future::{Future, pending};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::SystemTime;

use prost_types::{Duration as ProtoDuration, Timestamp};
use rehydration_application::{
    AcceptedVersion, AdminApplicationService, ApplicationError, BundleSnapshotResult,
    CommandApplicationService, GetBundleSnapshotQuery, GetContextQuery, GetContextResult,
    GetGraphRelationshipsQuery, GetGraphRelationshipsResult, GetProjectionStatusQuery,
    GetProjectionStatusResult, GetRehydrationDiagnosticsQuery, GetRehydrationDiagnosticsResult,
    GraphNodeView, GraphRelationshipView, ProjectionStatusView, QueryApplicationService,
    RehydrateSessionQuery, RehydrateSessionResult, RehydrationApplication,
    RehydrationDiagnosticView, ReplayModeSelection, ReplayProjectionCommand,
    ReplayProjectionOutcome, ScopeValidation, UpdateContextCommand, UpdateContextUseCase,
    ValidateScopeQuery,
};
use rehydration_config::AppConfig;
use rehydration_domain::{BundleMetadata, RehydrationBundle};
use rehydration_ports::{ProjectionReader, SnapshotStore};
use rehydration_proto::v1alpha1::{
    BundleRenderFormat, BundleSection, BundleSnapshot, BundleVersion, CaseHeader, CommandMetadata,
    Decision, DecisionRelation, FILE_DESCRIPTOR_SET, GetBundleSnapshotRequest,
    GetBundleSnapshotResponse, GetContextRequest, GetContextResponse, GetGraphRelationshipsRequest,
    GetGraphRelationshipsResponse, GetProjectionStatusRequest, GetProjectionStatusResponse,
    GetRehydrationDiagnosticsRequest, GetRehydrationDiagnosticsResponse, GraphNode,
    GraphRelationship, Milestone, Phase, PlanHeader, ProjectionStatus, RehydrateSessionRequest,
    RehydrateSessionResponse, RehydrationBundle as ProtoRehydrationBundle, RehydrationDiagnostic,
    RehydrationStats, RenderedContext as ProtoRenderedContext, ReplayMode, ReplayProjectionRequest,
    ReplayProjectionResponse, RevisionPrecondition, RoleContextPack, ScopeValidationResult,
    TaskImpact, UpdateContextRequest, UpdateContextResponse, ValidateScopeRequest,
    ValidateScopeResponse, WorkItem,
    context_admin_service_server::{ContextAdminService, ContextAdminServiceServer},
    context_command_service_server::{ContextCommandService, ContextCommandServiceServer},
    context_query_service_server::{ContextQueryService, ContextQueryServiceServer},
};
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::{Request, Response, Status, transport::Server};

#[derive(Debug)]
pub struct GrpcServer<R, S> {
    bind_addr: String,
    query_application: Arc<QueryApplicationService<R, S>>,
    admin_application: Arc<AdminApplicationService<R>>,
    command_application: Arc<CommandApplicationService>,
    capability_name: &'static str,
}

impl<R, S> GrpcServer<R, S>
where
    R: ProjectionReader + Send + Sync + 'static,
    S: SnapshotStore + Send + Sync + 'static,
{
    pub fn new(config: AppConfig, projection_reader: R, snapshot_store: S) -> Self {
        let projection_reader = Arc::new(projection_reader);
        let snapshot_store = Arc::new(snapshot_store);
        let generator_version = env!("CARGO_PKG_VERSION");
        let update_context = Arc::new(UpdateContextUseCase::new(generator_version));

        Self {
            bind_addr: config.grpc_bind,
            query_application: Arc::new(QueryApplicationService::new(
                Arc::clone(&projection_reader),
                Arc::clone(&snapshot_store),
                generator_version,
            )),
            admin_application: Arc::new(AdminApplicationService::new(
                Arc::clone(&projection_reader),
                generator_version,
            )),
            command_application: Arc::new(CommandApplicationService::new(update_context)),
            capability_name: RehydrationApplication::capability_name(),
        }
    }

    pub fn describe(&self) -> String {
        format!(
            "grpc transport for {} on {}",
            self.capability_name, self.bind_addr
        )
    }

    pub fn bootstrap_request(&self) -> GetContextRequest {
        GetContextRequest {
            case_id: "bootstrap-case".to_string(),
            role: "system".to_string(),
            phase: Phase::Build as i32,
            work_item_id: String::new(),
            token_budget: 4096,
            requested_scopes: Vec::new(),
            render_format: BundleRenderFormat::Structured as i32,
            include_debug_sections: false,
        }
    }

    pub fn descriptor_set(&self) -> &'static [u8] {
        FILE_DESCRIPTOR_SET
    }

    pub fn query_service(&self) -> QueryGrpcService<R, S> {
        QueryGrpcService::new(Arc::clone(&self.query_application))
    }

    pub fn command_service(&self) -> CommandGrpcService {
        CommandGrpcService::new(Arc::clone(&self.command_application))
    }

    pub fn admin_service(&self) -> AdminGrpcService<R> {
        AdminGrpcService::new(Arc::clone(&self.admin_application))
    }

    pub async fn warmup_bundle(&self) -> Result<RehydrationBundle, ApplicationError> {
        self.query_application.warmup_bundle().await
    }

    pub async fn run(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let bind_addr: SocketAddr = self.bind_addr.parse()?;
        let listener = TcpListener::bind(bind_addr).await?;

        self.serve_with_listener_shutdown(listener, pending()).await
    }

    pub async fn serve_with_listener_shutdown<F>(
        self,
        listener: TcpListener,
        shutdown: F,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        Server::builder()
            .add_service(ContextQueryServiceServer::new(self.query_service()))
            .add_service(ContextCommandServiceServer::new(self.command_service()))
            .add_service(ContextAdminServiceServer::new(self.admin_service()))
            .serve_with_incoming_shutdown(TcpListenerStream::new(listener), shutdown)
            .await?;

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct QueryGrpcService<R, S> {
    application: Arc<QueryApplicationService<R, S>>,
}

impl<R, S> QueryGrpcService<R, S> {
    pub fn new(application: Arc<QueryApplicationService<R, S>>) -> Self {
        Self { application }
    }
}

#[tonic::async_trait]
impl<R, S> ContextQueryService for QueryGrpcService<R, S>
where
    R: ProjectionReader + Send + Sync + 'static,
    S: SnapshotStore + Send + Sync + 'static,
{
    async fn get_context(
        &self,
        request: Request<GetContextRequest>,
    ) -> Result<Response<GetContextResponse>, Status> {
        let request = request.into_inner();
        let result = self
            .application
            .get_context(GetContextQuery {
                case_id: request.case_id,
                role: request.role,
                requested_scopes: request.requested_scopes,
            })
            .await
            .map_err(map_application_error)?;

        Ok(Response::new(GetContextResponse {
            bundle: Some(proto_bundle_from_single_role(&result.bundle)),
            rendered: Some(proto_rendered_context_from_result(&result)),
            scope_validation: Some(proto_scope_validation(&result.scope_validation)),
            served_at: Some(timestamp_from(result.served_at)),
        }))
    }

    async fn rehydrate_session(
        &self,
        request: Request<RehydrateSessionRequest>,
    ) -> Result<Response<RehydrateSessionResponse>, Status> {
        let request = request.into_inner();
        let result = self
            .application
            .rehydrate_session(RehydrateSessionQuery {
                case_id: request.case_id,
                roles: request.roles,
                persist_snapshot: request.persist_snapshot,
                timeline_window: request.timeline_window,
            })
            .await
            .map_err(map_application_error)?;

        Ok(Response::new(proto_rehydrate_session_response(&result)))
    }

    async fn validate_scope(
        &self,
        request: Request<ValidateScopeRequest>,
    ) -> Result<Response<ValidateScopeResponse>, Status> {
        let request = request.into_inner();
        let result = self.application.validate_scope(ValidateScopeQuery {
            required_scopes: request.required_scopes,
            provided_scopes: request.provided_scopes,
        });

        Ok(Response::new(ValidateScopeResponse {
            result: Some(proto_scope_validation(&result)),
        }))
    }
}

#[derive(Debug, Clone)]
pub struct AdminGrpcService<R> {
    application: Arc<AdminApplicationService<R>>,
}

impl<R> AdminGrpcService<R> {
    pub fn new(application: Arc<AdminApplicationService<R>>) -> Self {
        Self { application }
    }
}

#[tonic::async_trait]
impl<R> ContextAdminService for AdminGrpcService<R>
where
    R: ProjectionReader + Send + Sync + 'static,
{
    async fn get_projection_status(
        &self,
        request: Request<GetProjectionStatusRequest>,
    ) -> Result<Response<GetProjectionStatusResponse>, Status> {
        let request = request.into_inner();
        let result = self
            .application
            .get_projection_status(GetProjectionStatusQuery {
                consumer_names: request.consumer_names,
            });

        Ok(Response::new(proto_projection_status_response(&result)))
    }

    async fn replay_projection(
        &self,
        request: Request<ReplayProjectionRequest>,
    ) -> Result<Response<ReplayProjectionResponse>, Status> {
        let request = request.into_inner();
        let result = self
            .application
            .replay_projection(ReplayProjectionCommand {
                consumer_name: request.consumer_name,
                stream_name: request.stream_name,
                starting_after: trim_to_option(request.starting_after),
                max_events: request.max_events,
                replay_mode: map_replay_mode(request.replay_mode),
                requested_by: trim_to_option(request.requested_by),
            })
            .map_err(map_application_error)?;

        Ok(Response::new(proto_replay_projection_response(&result)))
    }

    async fn get_bundle_snapshot(
        &self,
        request: Request<GetBundleSnapshotRequest>,
    ) -> Result<Response<GetBundleSnapshotResponse>, Status> {
        let request = request.into_inner();
        let result = self
            .application
            .get_bundle_snapshot(GetBundleSnapshotQuery {
                case_id: request.case_id,
                role: request.role,
            })
            .await
            .map_err(map_application_error)?;

        Ok(Response::new(proto_bundle_snapshot_response(&result)))
    }

    async fn get_graph_relationships(
        &self,
        request: Request<GetGraphRelationshipsRequest>,
    ) -> Result<Response<GetGraphRelationshipsResponse>, Status> {
        let request = request.into_inner();
        let result = self
            .application
            .get_graph_relationships(GetGraphRelationshipsQuery {
                node_id: request.node_id,
                node_kind: trim_to_option(request.node_kind),
                depth: request.depth,
                include_reverse_edges: request.include_reverse_edges,
            })
            .map_err(map_application_error)?;

        Ok(Response::new(proto_graph_relationships_response(&result)))
    }

    async fn get_rehydration_diagnostics(
        &self,
        request: Request<GetRehydrationDiagnosticsRequest>,
    ) -> Result<Response<GetRehydrationDiagnosticsResponse>, Status> {
        let request = request.into_inner();
        let phase = Phase::try_from(request.phase)
            .unwrap_or(Phase::Unspecified)
            .as_str_name()
            .to_string();
        let result = self
            .application
            .get_rehydration_diagnostics(GetRehydrationDiagnosticsQuery {
                case_id: request.case_id,
                roles: request.roles,
                phase: trim_to_option(phase),
            })
            .await
            .map_err(map_application_error)?;

        Ok(Response::new(proto_rehydration_diagnostics_response(
            &result,
        )))
    }
}

#[derive(Debug, Clone)]
pub struct CommandGrpcService {
    application: Arc<CommandApplicationService>,
}

impl CommandGrpcService {
    pub fn new(application: Arc<CommandApplicationService>) -> Self {
        Self { application }
    }
}

#[tonic::async_trait]
impl ContextCommandService for CommandGrpcService {
    async fn update_context(
        &self,
        request: Request<UpdateContextRequest>,
    ) -> Result<Response<UpdateContextResponse>, Status> {
        let request = request.into_inner();
        let metadata = request.metadata.unwrap_or(CommandMetadata {
            idempotency_key: String::new(),
            correlation_id: String::new(),
            causation_id: String::new(),
            requested_by: String::new(),
            requested_at: None,
        });
        let precondition = request.precondition.unwrap_or(RevisionPrecondition {
            expected_revision: 0,
            expected_content_hash: String::new(),
        });

        let outcome = self
            .application
            .update_context(UpdateContextCommand {
                case_id: request.case_id,
                role: request.role,
                work_item_id: request.work_item_id,
                changes: request
                    .changes
                    .into_iter()
                    .map(|change| rehydration_application::UpdateContextChange {
                        operation: change.operation().as_str_name().to_string(),
                        entity_kind: change.entity_kind,
                        entity_id: change.entity_id,
                        payload_json: change.payload_json,
                        reason: change.reason,
                        scopes: change.scopes,
                    })
                    .collect(),
                expected_revision: (precondition.expected_revision != 0)
                    .then_some(precondition.expected_revision),
                expected_content_hash: (!precondition.expected_content_hash.is_empty())
                    .then_some(precondition.expected_content_hash),
                idempotency_key: (!metadata.idempotency_key.is_empty())
                    .then_some(metadata.idempotency_key),
                requested_by: (!metadata.requested_by.is_empty()).then_some(metadata.requested_by),
                persist_snapshot: request.persist_snapshot,
            })
            .map_err(map_application_error)?;

        Ok(Response::new(UpdateContextResponse {
            accepted_version: Some(proto_accepted_version(&outcome.accepted_version)),
            warnings: outcome.warnings,
            snapshot_persisted: outcome.snapshot_persisted,
            snapshot_id: outcome.snapshot_id.unwrap_or_default(),
        }))
    }
}

fn proto_rehydrate_session_response(result: &RehydrateSessionResult) -> RehydrateSessionResponse {
    let decisions = result
        .bundles
        .iter()
        .map(|bundle| bundle.pack().decisions().len() as u32)
        .sum();
    let decision_relations = result
        .bundles
        .iter()
        .map(|bundle| bundle.pack().decision_relations().len() as u32)
        .sum();
    let impacts = result
        .bundles
        .iter()
        .map(|bundle| bundle.pack().impacts().len() as u32)
        .sum();
    let milestones = result
        .bundles
        .iter()
        .map(|bundle| bundle.pack().milestones().len() as u32)
        .sum();

    RehydrateSessionResponse {
        bundle: Some(ProtoRehydrationBundle {
            case_id: result.case_id.clone(),
            packs: result
                .bundles
                .iter()
                .map(proto_role_pack_from_domain)
                .collect(),
            stats: Some(RehydrationStats {
                roles: result.bundles.len() as u32,
                decisions,
                decision_relations,
                impacts,
                milestones,
                timeline_events: result.timeline_events,
            }),
            version: Some(proto_bundle_version(&result.version)),
        }),
        snapshot_persisted: result.snapshot_persisted,
        snapshot_id: result.snapshot_id.clone().unwrap_or_default(),
        generated_at: Some(timestamp_from(result.generated_at)),
    }
}

fn proto_projection_status_response(
    result: &GetProjectionStatusResult,
) -> GetProjectionStatusResponse {
    GetProjectionStatusResponse {
        projections: result
            .projections
            .iter()
            .map(proto_projection_status)
            .collect(),
        observed_at: Some(timestamp_from(result.observed_at)),
    }
}

fn proto_replay_projection_response(result: &ReplayProjectionOutcome) -> ReplayProjectionResponse {
    ReplayProjectionResponse {
        replay_id: result.replay_id.clone(),
        consumer_name: result.consumer_name.clone(),
        replay_mode: proto_replay_mode(result.replay_mode) as i32,
        accepted_events: result.accepted_events,
        requested_at: Some(timestamp_from(result.requested_at)),
    }
}

fn proto_bundle_snapshot_response(result: &BundleSnapshotResult) -> GetBundleSnapshotResponse {
    GetBundleSnapshotResponse {
        snapshot: Some(BundleSnapshot {
            snapshot_id: result.snapshot_id.clone(),
            case_id: result.case_id.clone(),
            role: result.role.clone(),
            bundle: Some(proto_bundle_from_single_role(&result.bundle)),
            created_at: Some(timestamp_from(result.created_at)),
            expires_at: Some(timestamp_from(result.expires_at)),
            ttl: Some(proto_duration(result.ttl_seconds)),
        }),
    }
}

fn proto_graph_relationships_response(
    result: &GetGraphRelationshipsResult,
) -> GetGraphRelationshipsResponse {
    GetGraphRelationshipsResponse {
        root: Some(proto_graph_node(&result.root)),
        neighbors: result.neighbors.iter().map(proto_graph_node).collect(),
        relationships: result
            .relationships
            .iter()
            .map(proto_graph_relationship)
            .collect(),
        observed_at: Some(timestamp_from(result.observed_at)),
    }
}

fn proto_rehydration_diagnostics_response(
    result: &GetRehydrationDiagnosticsResult,
) -> GetRehydrationDiagnosticsResponse {
    GetRehydrationDiagnosticsResponse {
        diagnostics: result.diagnostics.iter().map(proto_diagnostic).collect(),
        observed_at: Some(timestamp_from(result.observed_at)),
    }
}

fn proto_bundle_from_single_role(bundle: &RehydrationBundle) -> ProtoRehydrationBundle {
    ProtoRehydrationBundle {
        case_id: bundle.case_id().as_str().to_string(),
        packs: vec![proto_role_pack_from_domain(bundle)],
        stats: Some(RehydrationStats {
            roles: 1,
            decisions: bundle.pack().decisions().len() as u32,
            decision_relations: bundle.pack().decision_relations().len() as u32,
            impacts: bundle.pack().impacts().len() as u32,
            milestones: bundle.pack().milestones().len() as u32,
            timeline_events: 0,
        }),
        version: Some(proto_bundle_version(bundle.metadata())),
    }
}

fn proto_role_pack_from_domain(bundle: &RehydrationBundle) -> RoleContextPack {
    let pack = bundle.pack();
    let case_header = pack.case_header();

    RoleContextPack {
        role: pack.role().as_str().to_string(),
        case_header: Some(CaseHeader {
            case_id: case_header.case_id().as_str().to_string(),
            title: case_header.title().to_string(),
            summary: case_header.summary().to_string(),
            status: case_header.status().to_string(),
            created_at: Some(timestamp_from(case_header.created_at())),
            created_by: case_header.created_by().to_string(),
        }),
        plan_header: pack.plan_header().map(|plan| PlanHeader {
            plan_id: plan.plan_id().to_string(),
            revision: plan.revision(),
            status: plan.status().to_string(),
            work_items_total: plan.work_items_total(),
            work_items_completed: plan.work_items_completed(),
        }),
        work_items: pack
            .work_items()
            .iter()
            .map(|work_item| WorkItem {
                work_item_id: work_item.work_item_id().to_string(),
                title: work_item.title().to_string(),
                summary: work_item.summary().to_string(),
                role: work_item.role().to_string(),
                phase: work_item.phase().to_string(),
                status: work_item.status().to_string(),
                dependency_ids: work_item.dependency_ids().to_vec(),
                priority: work_item.priority(),
            })
            .collect(),
        decisions: pack
            .decisions()
            .iter()
            .map(|decision| Decision {
                decision_id: decision.decision_id().to_string(),
                title: decision.title().to_string(),
                rationale: decision.rationale().to_string(),
                status: decision.status().to_string(),
                owner: decision.owner().to_string(),
                decided_at: Some(timestamp_from(decision.decided_at())),
            })
            .collect(),
        decision_relations: pack
            .decision_relations()
            .iter()
            .map(|relation| DecisionRelation {
                source_decision_id: relation.source_decision_id().to_string(),
                target_decision_id: relation.target_decision_id().to_string(),
                relation_type: relation.relation_type().to_string(),
            })
            .collect(),
        impacts: pack
            .impacts()
            .iter()
            .map(|impact| TaskImpact {
                decision_id: impact.decision_id().to_string(),
                work_item_id: impact.work_item_id().to_string(),
                title: impact.title().to_string(),
                impact_type: impact.impact_type().to_string(),
            })
            .collect(),
        milestones: pack
            .milestones()
            .iter()
            .map(|milestone| Milestone {
                milestone_type: milestone.milestone_type().to_string(),
                description: milestone.description().to_string(),
                occurred_at: Some(timestamp_from(milestone.occurred_at())),
                actor: milestone.actor().to_string(),
            })
            .collect(),
        latest_summary: pack.latest_summary().to_string(),
        token_budget_hint: pack.token_budget_hint(),
    }
}

fn proto_rendered_context_from_result(result: &GetContextResult) -> ProtoRenderedContext {
    ProtoRenderedContext {
        format: BundleRenderFormat::Structured as i32,
        content: result.rendered.content.clone(),
        token_count: result.rendered.token_count,
        sections: result
            .rendered
            .sections
            .iter()
            .enumerate()
            .map(|(index, section)| BundleSection {
                key: format!("section_{index}"),
                title: format!("Section {}", index + 1),
                content: section.clone(),
                token_count: section.split_whitespace().count() as u32,
                scopes: result.scope_validation.provided_scopes.clone(),
            })
            .collect(),
    }
}

fn proto_scope_validation(result: &ScopeValidation) -> ScopeValidationResult {
    ScopeValidationResult {
        allowed: result.allowed,
        required_scopes: result.required_scopes.clone(),
        provided_scopes: result.provided_scopes.clone(),
        missing_scopes: result.missing_scopes.clone(),
        extra_scopes: result.extra_scopes.clone(),
        reason: result.reason.clone(),
        diagnostics: result.diagnostics.clone(),
    }
}

fn proto_projection_status(view: &ProjectionStatusView) -> ProjectionStatus {
    ProjectionStatus {
        consumer_name: view.consumer_name.clone(),
        stream_name: view.stream_name.clone(),
        projection_watermark: view.projection_watermark.clone(),
        processed_events: view.processed_events,
        pending_events: view.pending_events,
        last_event_at: Some(timestamp_from(view.last_event_at)),
        updated_at: Some(timestamp_from(view.updated_at)),
        healthy: view.healthy,
        warnings: view.warnings.clone(),
    }
}

fn proto_graph_node(node: &GraphNodeView) -> GraphNode {
    GraphNode {
        node_id: node.node_id.clone(),
        node_kind: node.node_kind.clone(),
        title: node.title.clone(),
        labels: node.labels.clone(),
        properties: node.properties.clone().into_iter().collect(),
    }
}

fn proto_graph_relationship(relationship: &GraphRelationshipView) -> GraphRelationship {
    GraphRelationship {
        source_node_id: relationship.source_node_id.clone(),
        target_node_id: relationship.target_node_id.clone(),
        relationship_type: relationship.relationship_type.clone(),
        properties: relationship.properties.clone().into_iter().collect(),
    }
}

fn proto_diagnostic(diagnostic: &RehydrationDiagnosticView) -> RehydrationDiagnostic {
    RehydrationDiagnostic {
        role: diagnostic.role.clone(),
        version: Some(proto_bundle_version(&diagnostic.version)),
        selected_decisions: diagnostic.selected_decisions,
        selected_impacts: diagnostic.selected_impacts,
        selected_milestones: diagnostic.selected_milestones,
        estimated_tokens: diagnostic.estimated_tokens,
        notes: diagnostic.notes.clone(),
    }
}

fn proto_accepted_version(version: &AcceptedVersion) -> BundleVersion {
    BundleVersion {
        revision: version.revision,
        content_hash: version.content_hash.clone(),
        schema_version: "v1alpha1".to_string(),
        projection_watermark: format!("rev-{}", version.revision),
        generated_at: Some(timestamp_from(SystemTime::now())),
        generator_version: version.generator_version.clone(),
    }
}

fn proto_bundle_version(metadata: &BundleMetadata) -> BundleVersion {
    BundleVersion {
        revision: metadata.revision,
        content_hash: metadata.content_hash.clone(),
        schema_version: "v1alpha1".to_string(),
        projection_watermark: format!("rev-{}", metadata.revision),
        generated_at: Some(timestamp_from(SystemTime::now())),
        generator_version: metadata.generator_version.clone(),
    }
}

fn map_replay_mode(value: i32) -> ReplayModeSelection {
    match ReplayMode::try_from(value).unwrap_or(ReplayMode::DryRun) {
        ReplayMode::DryRun => ReplayModeSelection::DryRun,
        ReplayMode::Rebuild => ReplayModeSelection::Rebuild,
        ReplayMode::Unspecified => ReplayModeSelection::DryRun,
    }
}

fn proto_replay_mode(value: ReplayModeSelection) -> ReplayMode {
    match value {
        ReplayModeSelection::DryRun => ReplayMode::DryRun,
        ReplayModeSelection::Rebuild => ReplayMode::Rebuild,
    }
}

fn map_application_error(error: ApplicationError) -> Status {
    match error {
        ApplicationError::Domain(domain_error) => {
            Status::invalid_argument(domain_error.to_string())
        }
        ApplicationError::Ports(port_error) => match port_error {
            rehydration_ports::PortError::InvalidState(message) => {
                Status::failed_precondition(message)
            }
            rehydration_ports::PortError::Unavailable(message) => Status::unavailable(message),
        },
        ApplicationError::Validation(message) => Status::invalid_argument(message),
    }
}

fn timestamp_from(value: SystemTime) -> Timestamp {
    Timestamp::from(value)
}

fn proto_duration(seconds: u64) -> ProtoDuration {
    ProtoDuration {
        seconds: seconds as i64,
        nanos: 0,
    }
}

fn trim_to_option(value: String) -> Option<String> {
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
    use std::time::{Duration, SystemTime};

    use rehydration_application::{
        AcceptedVersion, AdminApplicationService, ApplicationError, BundleSnapshotResult,
        CommandApplicationService, GetGraphRelationshipsResult, GetProjectionStatusResult,
        GetRehydrationDiagnosticsResult, GraphNodeView, GraphRelationshipView,
        ProjectionStatusView, QueryApplicationService, RehydrateSessionResult,
        RehydrationDiagnosticView, ReplayModeSelection, ReplayProjectionOutcome,
    };
    use rehydration_domain::{
        BundleMetadata, CaseHeader, CaseId, Decision, Milestone, PlanHeader, RehydrationBundle,
        Role, RoleContextPack, TaskImpact, WorkItem,
    };
    use rehydration_ports::{PortError, ProjectionReader, SnapshotStore};
    use rehydration_proto::v1alpha1::{
        BundleRenderFormat, ContextChange, ContextChangeOperation, GetBundleSnapshotRequest,
        GetContextRequest, GetProjectionStatusRequest, GetRehydrationDiagnosticsRequest, Phase,
        UpdateContextRequest, ValidateScopeRequest,
        context_admin_service_server::ContextAdminService,
        context_command_service_server::ContextCommandService,
        context_query_service_server::ContextQueryService,
    };
    use tonic::Request;

    use super::{
        AdminGrpcService, CommandGrpcService, GrpcServer, QueryGrpcService, map_application_error,
        map_replay_mode, proto_accepted_version, proto_bundle_snapshot_response,
        proto_bundle_version, proto_duration, proto_graph_relationships_response,
        proto_projection_status_response, proto_rehydrate_session_response,
        proto_rehydration_diagnostics_response, proto_replay_mode,
        proto_replay_projection_response, trim_to_option,
    };

    struct EmptyProjectionReader;

    impl ProjectionReader for EmptyProjectionReader {
        async fn load_pack(
            &self,
            _case_id: &CaseId,
            _role: &Role,
        ) -> Result<Option<RoleContextPack>, PortError> {
            Ok(None)
        }
    }

    struct NoopSnapshotStore;

    impl SnapshotStore for NoopSnapshotStore {
        async fn save_bundle(&self, _bundle: &RehydrationBundle) -> Result<(), PortError> {
            Ok(())
        }
    }

    #[test]
    fn describe_mentions_bind_address() {
        let config = rehydration_config::AppConfig {
            service_name: "rehydration-kernel".to_string(),
            grpc_bind: "127.0.0.1:50054".to_string(),
            admin_bind: "127.0.0.1:8080".to_string(),
            graph_uri: "neo4j://localhost:7687".to_string(),
            snapshot_uri: "redis://localhost:6379".to_string(),
            events_subject_prefix: "rehydration".to_string(),
        };
        let server = GrpcServer::new(config, EmptyProjectionReader, NoopSnapshotStore);

        assert!(server.describe().contains("127.0.0.1:50054"));
        assert_eq!(server.bootstrap_request().case_id, "bootstrap-case");
        let descriptor_set = std::hint::black_box(server.descriptor_set());
        assert!(!descriptor_set.is_empty());
    }

    #[tokio::test]
    async fn query_service_returns_rendered_context() {
        let application = Arc::new(QueryApplicationService::new(
            Arc::new(EmptyProjectionReader),
            Arc::new(NoopSnapshotStore),
            "0.1.0",
        ));
        let service = QueryGrpcService::new(application);

        let response = service
            .get_context(Request::new(GetContextRequest {
                case_id: "case-123".to_string(),
                role: "developer".to_string(),
                phase: Phase::Build as i32,
                work_item_id: String::new(),
                token_budget: 1024,
                requested_scopes: vec!["decisions".to_string()],
                render_format: BundleRenderFormat::Structured as i32,
                include_debug_sections: false,
            }))
            .await
            .expect("get context should succeed")
            .into_inner();

        assert_eq!(
            response
                .bundle
                .as_ref()
                .expect("bundle should exist")
                .case_id,
            "case-123"
        );
        assert!(
            response
                .rendered
                .as_ref()
                .expect("rendered context should exist")
                .content
                .contains("case-123")
        );
    }

    #[tokio::test]
    async fn query_service_validates_scope_diffs() {
        let application = Arc::new(QueryApplicationService::new(
            Arc::new(EmptyProjectionReader),
            Arc::new(NoopSnapshotStore),
            "0.1.0",
        ));
        let service = QueryGrpcService::new(application);

        let response = service
            .validate_scope(Request::new(ValidateScopeRequest {
                role: "developer".to_string(),
                phase: Phase::Build as i32,
                required_scopes: vec!["decisions".to_string()],
                provided_scopes: vec!["milestones".to_string()],
            }))
            .await
            .expect("validate scope should succeed")
            .into_inner();

        let result = response.result.expect("validation result should exist");
        assert!(!result.allowed);
        assert_eq!(result.missing_scopes, vec!["decisions".to_string()]);
    }

    #[tokio::test]
    async fn command_service_accepts_update_context() {
        let service = CommandGrpcService::new(Arc::new(CommandApplicationService::new(Arc::new(
            rehydration_application::UpdateContextUseCase::new("0.1.0"),
        ))));

        let response = service
            .update_context(Request::new(UpdateContextRequest {
                case_id: "case-123".to_string(),
                role: "developer".to_string(),
                work_item_id: "task-7".to_string(),
                changes: vec![ContextChange {
                    operation: ContextChangeOperation::Update as i32,
                    entity_kind: "decision".to_string(),
                    entity_id: "decision-9".to_string(),
                    payload_json: "{\"status\":\"accepted\"}".to_string(),
                    reason: "refined".to_string(),
                    scopes: vec!["decisions".to_string()],
                }],
                metadata: None,
                precondition: None,
                persist_snapshot: true,
            }))
            .await
            .expect("update context should succeed")
            .into_inner();

        assert_eq!(
            response
                .accepted_version
                .as_ref()
                .expect("accepted version should exist")
                .revision,
            1
        );
        assert!(response.snapshot_persisted);
        assert_eq!(response.snapshot_id, "snapshot:case-123:developer");
    }

    #[tokio::test]
    async fn admin_service_returns_snapshot_and_status() {
        let application = Arc::new(AdminApplicationService::new(
            Arc::new(EmptyProjectionReader),
            "0.1.0",
        ));
        let service = AdminGrpcService::new(application);

        let status = service
            .get_projection_status(Request::new(GetProjectionStatusRequest {
                consumer_names: vec!["context-projection".to_string()],
            }))
            .await
            .expect("projection status should succeed")
            .into_inner();
        assert_eq!(status.projections.len(), 1);
        assert_eq!(status.projections[0].consumer_name, "context-projection");

        let snapshot = service
            .get_bundle_snapshot(Request::new(GetBundleSnapshotRequest {
                case_id: "case-123".to_string(),
                role: "developer".to_string(),
            }))
            .await
            .expect("bundle snapshot should succeed")
            .into_inner()
            .snapshot
            .expect("snapshot should exist");
        assert_eq!(snapshot.case_id, "case-123");
        assert_eq!(snapshot.role, "developer");
    }

    #[tokio::test]
    async fn admin_service_returns_diagnostics_per_role() {
        let application = Arc::new(AdminApplicationService::new(
            Arc::new(EmptyProjectionReader),
            "0.1.0",
        ));
        let service = AdminGrpcService::new(application);

        let response = service
            .get_rehydration_diagnostics(Request::new(GetRehydrationDiagnosticsRequest {
                case_id: "case-123".to_string(),
                roles: vec!["developer".to_string(), "reviewer".to_string()],
                phase: Phase::Build as i32,
            }))
            .await
            .expect("diagnostics should succeed")
            .into_inner();

        assert_eq!(response.diagnostics.len(), 2);
        assert_eq!(response.diagnostics[0].role, "developer");
        assert_eq!(
            response.diagnostics[0]
                .version
                .as_ref()
                .expect("version should exist")
                .schema_version,
            "v1alpha1"
        );
    }

    #[test]
    fn proto_role_pack_mapping_uses_structured_domain_data() {
        let case_id = CaseId::new("case-123").expect("case id is valid");
        let role = Role::new("developer").expect("role is valid");
        let bundle = RehydrationBundle::from_pack(
            RoleContextPack::new(
                role.clone(),
                CaseHeader::new(
                    case_id.clone(),
                    "Case 123",
                    "Projection-backed summary",
                    "ACTIVE",
                    SystemTime::UNIX_EPOCH,
                    "planner",
                ),
                Some(PlanHeader::new("plan-123", 4, "ACTIVE", 1, 0)),
                vec![WorkItem::new(
                    "task-1",
                    "Implement projection model",
                    "Ship a real query path",
                    role.as_str(),
                    "PHASE_BUILD",
                    "READY",
                    Vec::new(),
                    1,
                )],
                vec![Decision::new(
                    "decision-1",
                    "Use RoleContextPack",
                    "Stop inventing transport data",
                    "ACCEPTED",
                    "platform",
                    SystemTime::UNIX_EPOCH,
                )],
                Vec::new(),
                vec![TaskImpact::new(
                    "decision-1",
                    "task-1",
                    "Mapping now uses real work items",
                    "DIRECT",
                )],
                vec![Milestone::new(
                    "PHASE_TRANSITIONED",
                    "Entered build phase",
                    SystemTime::UNIX_EPOCH,
                    "system",
                )],
                "Projection-backed summary",
                3072,
            ),
            BundleMetadata::initial("0.1.0"),
        );

        let proto = super::proto_role_pack_from_domain(&bundle);

        assert_eq!(proto.role, "developer");
        assert_eq!(
            proto.case_header.expect("case header should exist").summary,
            "Projection-backed summary"
        );
        assert_eq!(proto.work_items.len(), 1);
        assert_eq!(proto.decisions.len(), 1);
        assert_eq!(proto.impacts.len(), 1);
        assert_eq!(proto.milestones.len(), 1);
        assert_eq!(proto.token_budget_hint, 3072);
    }

    #[test]
    fn helper_mappers_cover_projection_replay_and_diagnostics() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let projection_status = proto_projection_status_response(&GetProjectionStatusResult {
            projections: vec![ProjectionStatusView {
                consumer_name: "context-projection".to_string(),
                stream_name: "planning.story.created".to_string(),
                projection_watermark: "evt-42".to_string(),
                processed_events: 12,
                pending_events: 1,
                last_event_at: now,
                updated_at: now,
                healthy: true,
                warnings: vec!["lagging".to_string()],
            }],
            observed_at: now,
        });
        assert_eq!(
            projection_status.projections[0].consumer_name,
            "context-projection"
        );

        let replay = proto_replay_projection_response(&ReplayProjectionOutcome {
            replay_id: "replay-1".to_string(),
            consumer_name: "context-projection".to_string(),
            replay_mode: ReplayModeSelection::Rebuild,
            accepted_events: 42,
            requested_at: now,
        });
        assert_eq!(
            replay.replay_mode,
            rehydration_proto::v1alpha1::ReplayMode::Rebuild as i32
        );

        let diagnostics =
            proto_rehydration_diagnostics_response(&GetRehydrationDiagnosticsResult {
                diagnostics: vec![RehydrationDiagnosticView {
                    role: "developer".to_string(),
                    version: BundleMetadata::initial("0.1.0"),
                    selected_decisions: 2,
                    selected_impacts: 3,
                    selected_milestones: 1,
                    estimated_tokens: 256,
                    notes: vec!["ok".to_string()],
                }],
                observed_at: now,
            });
        assert_eq!(diagnostics.diagnostics[0].estimated_tokens, 256);

        let graph = proto_graph_relationships_response(&GetGraphRelationshipsResult {
            root: GraphNodeView {
                node_id: "root".to_string(),
                node_kind: "case".to_string(),
                title: "Case".to_string(),
                labels: vec!["Case".to_string()],
                properties: [("phase".to_string(), "build".to_string())]
                    .into_iter()
                    .collect(),
            },
            neighbors: vec![GraphNodeView {
                node_id: "task-1".to_string(),
                node_kind: "task".to_string(),
                title: "Task 1".to_string(),
                labels: vec!["Task".to_string()],
                properties: Default::default(),
            }],
            relationships: vec![GraphRelationshipView {
                source_node_id: "root".to_string(),
                target_node_id: "task-1".to_string(),
                relationship_type: "DEPENDS_ON".to_string(),
                properties: [("order".to_string(), "1".to_string())]
                    .into_iter()
                    .collect(),
            }],
            observed_at: now,
        });
        assert_eq!(graph.neighbors.len(), 1);
        assert_eq!(graph.relationships[0].relationship_type, "DEPENDS_ON");
    }

    #[test]
    fn helper_mappers_cover_versions_errors_and_trim_logic() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_100);
        let version = proto_bundle_version(&BundleMetadata {
            revision: 7,
            content_hash: "abc123".to_string(),
            generator_version: "0.1.0".to_string(),
        });
        let accepted = proto_accepted_version(&AcceptedVersion {
            revision: 8,
            content_hash: "xyz789".to_string(),
            generator_version: "0.1.0".to_string(),
        });

        assert_eq!(version.revision, 7);
        assert_eq!(accepted.projection_watermark, "rev-8");
        assert_eq!(proto_duration(30).seconds, 30);
        assert_eq!(
            trim_to_option("  value  ".to_string()),
            Some("value".to_string())
        );
        assert_eq!(trim_to_option("   ".to_string()), None);
        assert_eq!(
            map_application_error(ApplicationError::Validation("bad".to_string())).code(),
            tonic::Code::InvalidArgument
        );
        assert_eq!(
            map_application_error(ApplicationError::Ports(PortError::Unavailable(
                "down".to_string()
            )))
            .code(),
            tonic::Code::Unavailable
        );
        assert_eq!(proto_replay_mode(map_replay_mode(999)) as i32, 1);
        assert_eq!(proto_replay_mode(ReplayModeSelection::Rebuild) as i32, 2);

        let response = proto_rehydrate_session_response(&RehydrateSessionResult {
            case_id: "case-123".to_string(),
            bundles: vec![RehydrationBundle::new(
                CaseId::new("case-123").expect("case id is valid"),
                Role::new("developer").expect("role is valid"),
                vec!["section one".to_string()],
                BundleMetadata::initial("0.1.0"),
            )],
            timeline_events: 9,
            version: BundleMetadata::initial("0.1.0"),
            snapshot_persisted: true,
            snapshot_id: Some("snapshot:case-123:developer".to_string()),
            generated_at: now,
        });
        assert_eq!(
            response
                .bundle
                .expect("bundle should exist")
                .stats
                .expect("stats should exist")
                .timeline_events,
            9
        );

        let snapshot = proto_bundle_snapshot_response(&BundleSnapshotResult {
            snapshot_id: "snapshot:case-123:developer".to_string(),
            case_id: "case-123".to_string(),
            role: "developer".to_string(),
            bundle: RehydrationBundle::empty(
                CaseId::new("case-123").expect("case id is valid"),
                Role::new("developer").expect("role is valid"),
                "0.1.0",
            ),
            created_at: now,
            expires_at: now + Duration::from_secs(900),
            ttl_seconds: 900,
        });
        assert_eq!(
            snapshot
                .snapshot
                .expect("snapshot should exist")
                .ttl
                .expect("ttl should exist")
                .seconds,
            900
        );

        let invalid_argument = map_application_error(ApplicationError::Domain(
            rehydration_domain::DomainError::EmptyValue("case_id"),
        ));
        assert_eq!(invalid_argument.code(), tonic::Code::InvalidArgument);
    }
}
