use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use prost_types::{Duration as ProtoDuration, Timestamp};
use rehydration_application::{
    AcceptedVersion, ApplicationError, GetContextResult, GetContextUseCase,
    RehydrateSessionUseCase, RehydrationApplication, ScopeValidation, UpdateContextCommand,
    UpdateContextUseCase, ValidateScopeUseCase,
};
use rehydration_config::AppConfig;
use rehydration_domain::{BundleMetadata, CaseId, RehydrationBundle, Role};
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
use tonic::{Request, Response, Status, transport::Server};

#[derive(Debug)]
pub struct GrpcServer<R, S> {
    bind_addr: String,
    projection_reader: Arc<R>,
    snapshot_store: Arc<S>,
    update_context: Arc<UpdateContextUseCase>,
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

        Self {
            bind_addr: config.grpc_bind,
            projection_reader,
            snapshot_store,
            update_context: Arc::new(UpdateContextUseCase::new(env!("CARGO_PKG_VERSION"))),
            capability_name: RehydrationApplication::capability_name(),
        }
    }

    pub fn describe(&self) -> String {
        format!(
            "grpc transport placeholder for {} on {}",
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
        QueryGrpcService::new(
            Arc::clone(&self.projection_reader),
            Arc::clone(&self.snapshot_store),
            env!("CARGO_PKG_VERSION"),
        )
    }

    pub fn command_service(&self) -> CommandGrpcService {
        CommandGrpcService::new(Arc::clone(&self.update_context))
    }

    pub fn admin_service(&self) -> AdminGrpcService<R> {
        AdminGrpcService::new(
            Arc::clone(&self.projection_reader),
            env!("CARGO_PKG_VERSION"),
        )
    }

    pub fn warmup_bundle(&self) -> Result<RehydrationBundle, ApplicationError> {
        RehydrateSessionUseCase::new(
            Arc::clone(&self.projection_reader),
            Arc::clone(&self.snapshot_store),
            env!("CARGO_PKG_VERSION"),
        )
        .execute("bootstrap-case", "system")
    }

    pub async fn run(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let bind_addr: SocketAddr = self.bind_addr.parse()?;

        Server::builder()
            .add_service(ContextQueryServiceServer::new(self.query_service()))
            .add_service(ContextCommandServiceServer::new(self.command_service()))
            .add_service(ContextAdminServiceServer::new(self.admin_service()))
            .serve(bind_addr)
            .await?;

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct QueryGrpcService<R, S> {
    generator_version: &'static str,
    projection_reader: Arc<R>,
    snapshot_store: Arc<S>,
}

impl<R, S> QueryGrpcService<R, S> {
    pub fn new(
        projection_reader: Arc<R>,
        snapshot_store: Arc<S>,
        generator_version: &'static str,
    ) -> Self {
        Self {
            generator_version,
            projection_reader,
            snapshot_store,
        }
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
        let rehydrate = RehydrateSessionUseCase::new(
            Arc::clone(&self.projection_reader),
            Arc::clone(&self.snapshot_store),
            self.generator_version,
        );
        let result = GetContextUseCase::new(rehydrate)
            .execute(&request.case_id, &request.role, &request.requested_scopes)
            .map_err(map_application_error)?;

        Ok(Response::new(GetContextResponse {
            bundle: Some(proto_bundle_from_single_role(&result.bundle)),
            rendered: Some(proto_rendered_context_from_result(&result)),
            scope_validation: Some(proto_scope_validation(&result.scope_validation)),
            served_at: Some(now_timestamp()),
        }))
    }

    async fn rehydrate_session(
        &self,
        request: Request<RehydrateSessionRequest>,
    ) -> Result<Response<RehydrateSessionResponse>, Status> {
        let request = request.into_inner();
        if request.roles.is_empty() {
            return Err(Status::invalid_argument("roles cannot be empty"));
        }

        let rehydrate = RehydrateSessionUseCase::new(
            Arc::clone(&self.projection_reader),
            Arc::clone(&self.snapshot_store),
            self.generator_version,
        );

        let mut packs = Vec::with_capacity(request.roles.len());
        for role in &request.roles {
            let bundle = rehydrate
                .execute(&request.case_id, role)
                .map_err(map_application_error)?;
            packs.push(proto_role_pack_from_domain(&bundle));
        }
        let case_id = request.case_id.clone();
        let snapshot_id = if request.persist_snapshot {
            format!("snapshot:{}:{}", case_id, request.roles.join(","))
        } else {
            String::new()
        };

        let bundle = ProtoRehydrationBundle {
            case_id,
            packs,
            stats: Some(RehydrationStats {
                roles: request.roles.len() as u32,
                decisions: 0,
                decision_relations: 0,
                impacts: 0,
                milestones: 0,
                timeline_events: request.timeline_window,
            }),
            version: Some(proto_bundle_version(&BundleMetadata::initial(
                self.generator_version,
            ))),
        };

        Ok(Response::new(RehydrateSessionResponse {
            bundle: Some(bundle),
            snapshot_persisted: request.persist_snapshot,
            snapshot_id,
            generated_at: Some(now_timestamp()),
        }))
    }

    async fn validate_scope(
        &self,
        request: Request<ValidateScopeRequest>,
    ) -> Result<Response<ValidateScopeResponse>, Status> {
        let request = request.into_inner();
        let result =
            ValidateScopeUseCase::execute(&request.required_scopes, &request.provided_scopes);

        Ok(Response::new(ValidateScopeResponse {
            result: Some(proto_scope_validation(&result)),
        }))
    }
}

#[derive(Debug, Clone)]
pub struct AdminGrpcService<R> {
    generator_version: &'static str,
    projection_reader: Arc<R>,
}

impl<R> AdminGrpcService<R>
where
    R: ProjectionReader,
{
    pub fn new(projection_reader: Arc<R>, generator_version: &'static str) -> Self {
        Self {
            generator_version,
            projection_reader,
        }
    }

    fn load_or_empty_bundle(
        &self,
        case_id: &str,
        role: &str,
    ) -> Result<RehydrationBundle, ApplicationError> {
        let case_id = CaseId::new(case_id)?;
        let role = Role::new(role)?;

        match self.projection_reader.load_bundle(&case_id, &role)? {
            Some(bundle) => Ok(bundle),
            None => Ok(RehydrationBundle::empty(
                case_id,
                role,
                self.generator_version,
            )),
        }
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
        let consumer_names = if request.consumer_names.is_empty() {
            vec!["context-projection".to_string()]
        } else {
            request.consumer_names
        };

        Ok(Response::new(GetProjectionStatusResponse {
            projections: consumer_names
                .iter()
                .map(|consumer_name| proto_projection_status(consumer_name))
                .collect(),
            observed_at: Some(now_timestamp()),
        }))
    }

    async fn replay_projection(
        &self,
        request: Request<ReplayProjectionRequest>,
    ) -> Result<Response<ReplayProjectionResponse>, Status> {
        let request = request.into_inner();
        if request.consumer_name.is_empty() {
            return Err(Status::invalid_argument("consumer_name cannot be empty"));
        }
        if request.stream_name.is_empty() {
            return Err(Status::invalid_argument("stream_name cannot be empty"));
        }

        let replay_mode = ReplayMode::try_from(request.replay_mode).unwrap_or(ReplayMode::DryRun);

        Ok(Response::new(ReplayProjectionResponse {
            replay_id: format!("replay:{}:{}", request.consumer_name, request.stream_name),
            consumer_name: request.consumer_name,
            replay_mode: replay_mode as i32,
            accepted_events: request.max_events,
            requested_at: Some(now_timestamp()),
        }))
    }

    async fn get_bundle_snapshot(
        &self,
        request: Request<GetBundleSnapshotRequest>,
    ) -> Result<Response<GetBundleSnapshotResponse>, Status> {
        let request = request.into_inner();
        let bundle = self
            .load_or_empty_bundle(&request.case_id, &request.role)
            .map_err(map_application_error)?;

        Ok(Response::new(GetBundleSnapshotResponse {
            snapshot: Some(proto_bundle_snapshot(&bundle)),
        }))
    }

    async fn get_graph_relationships(
        &self,
        request: Request<GetGraphRelationshipsRequest>,
    ) -> Result<Response<GetGraphRelationshipsResponse>, Status> {
        let request = request.into_inner();
        if request.node_id.is_empty() {
            return Err(Status::invalid_argument("node_id cannot be empty"));
        }

        let node_kind = if request.node_kind.is_empty() {
            "unknown".to_string()
        } else {
            request.node_kind
        };

        let root = GraphNode {
            node_id: request.node_id.clone(),
            node_kind: node_kind.clone(),
            title: format!("{} {}", node_kind, request.node_id),
            labels: vec![node_kind.clone()],
            properties: HashMap::from([
                ("source".to_string(), "admin-placeholder".to_string()),
                ("depth".to_string(), request.depth.to_string()),
            ]),
        };

        let mut neighbors = Vec::new();
        let mut relationships = Vec::new();

        if request.depth > 0 {
            let child_id = format!("{}-neighbor-1", request.node_id);
            neighbors.push(GraphNode {
                node_id: child_id.clone(),
                node_kind: node_kind.clone(),
                title: format!("Related {}", child_id),
                labels: vec!["related".to_string()],
                properties: HashMap::from([("edge_direction".to_string(), "outbound".to_string())]),
            });
            relationships.push(GraphRelationship {
                source_node_id: request.node_id.clone(),
                target_node_id: child_id,
                relationship_type: "RELATES_TO".to_string(),
                properties: HashMap::new(),
            });
        }

        if request.include_reverse_edges {
            let reverse_id = format!("{}-neighbor-reverse", request.node_id);
            neighbors.push(GraphNode {
                node_id: reverse_id.clone(),
                node_kind,
                title: format!("Reverse {}", reverse_id),
                labels: vec!["reverse".to_string()],
                properties: HashMap::from([("edge_direction".to_string(), "inbound".to_string())]),
            });
            relationships.push(GraphRelationship {
                source_node_id: reverse_id,
                target_node_id: request.node_id,
                relationship_type: "INFLUENCES".to_string(),
                properties: HashMap::new(),
            });
        }

        Ok(Response::new(GetGraphRelationshipsResponse {
            root: Some(root),
            neighbors,
            relationships,
            observed_at: Some(now_timestamp()),
        }))
    }

    async fn get_rehydration_diagnostics(
        &self,
        request: Request<GetRehydrationDiagnosticsRequest>,
    ) -> Result<Response<GetRehydrationDiagnosticsResponse>, Status> {
        let request = request.into_inner();
        if request.roles.is_empty() {
            return Err(Status::invalid_argument("roles cannot be empty"));
        }

        let phase_name = Phase::try_from(request.phase)
            .unwrap_or(Phase::Unspecified)
            .as_str_name()
            .to_string();
        let diagnostics = request
            .roles
            .iter()
            .map(|role| {
                self.load_or_empty_bundle(&request.case_id, role)
                    .map(|bundle| RehydrationDiagnostic {
                        role: role.clone(),
                        version: Some(proto_bundle_version(bundle.metadata())),
                        selected_decisions: 0,
                        selected_impacts: 0,
                        selected_milestones: 0,
                        estimated_tokens: bundle
                            .sections()
                            .iter()
                            .map(|section| section.split_whitespace().count() as u32)
                            .sum(),
                        notes: vec![format!("phase={phase_name}")],
                    })
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(map_application_error)?;

        Ok(Response::new(GetRehydrationDiagnosticsResponse {
            diagnostics,
            observed_at: Some(now_timestamp()),
        }))
    }
}

#[derive(Debug, Clone)]
pub struct CommandGrpcService {
    update_context: Arc<UpdateContextUseCase>,
}

impl CommandGrpcService {
    pub fn new(update_context: Arc<UpdateContextUseCase>) -> Self {
        Self { update_context }
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
            .update_context
            .execute(UpdateContextCommand {
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
            snapshot_id: if outcome.snapshot_persisted {
                "snapshot:pending".to_string()
            } else {
                String::new()
            },
        }))
    }
}

fn proto_bundle_from_single_role(bundle: &RehydrationBundle) -> ProtoRehydrationBundle {
    ProtoRehydrationBundle {
        case_id: bundle.case_id().as_str().to_string(),
        packs: vec![proto_role_pack_from_domain(bundle)],
        stats: Some(RehydrationStats {
            roles: 1,
            decisions: 0,
            decision_relations: 0,
            impacts: 0,
            milestones: 0,
            timeline_events: 0,
        }),
        version: Some(proto_bundle_version(bundle.metadata())),
    }
}

fn proto_role_pack_from_domain(bundle: &RehydrationBundle) -> RoleContextPack {
    RoleContextPack {
        role: bundle.role().as_str().to_string(),
        case_header: Some(CaseHeader {
            case_id: bundle.case_id().as_str().to_string(),
            title: format!("Case {}", bundle.case_id().as_str()),
            summary: format!("Deterministic placeholder for {}", bundle.role().as_str()),
            status: "ACTIVE".to_string(),
            created_at: Some(now_timestamp()),
            created_by: "rehydration-kernel".to_string(),
        }),
        plan_header: Some(PlanHeader {
            plan_id: format!("plan:{}", bundle.case_id().as_str()),
            revision: bundle.metadata().revision,
            status: "PLACEHOLDER".to_string(),
            work_items_total: bundle.sections().len() as u32,
            work_items_completed: 0,
        }),
        work_items: bundle
            .sections()
            .iter()
            .enumerate()
            .map(|(index, section)| WorkItem {
                work_item_id: format!("section-{index}"),
                title: format!("Section {}", index + 1),
                summary: section.clone(),
                role: bundle.role().as_str().to_string(),
                phase: Phase::Build.as_str_name().to_string(),
                status: "READY".to_string(),
                dependency_ids: Vec::new(),
                priority: (index + 1) as u32,
            })
            .collect(),
        decisions: Vec::<Decision>::new(),
        decision_relations: Vec::<DecisionRelation>::new(),
        impacts: Vec::<TaskImpact>::new(),
        milestones: Vec::<Milestone>::new(),
        latest_summary: bundle.sections().join(" "),
        token_budget_hint: 4096,
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

fn proto_accepted_version(version: &AcceptedVersion) -> BundleVersion {
    BundleVersion {
        revision: version.revision,
        content_hash: version.content_hash.clone(),
        schema_version: "v1alpha1".to_string(),
        projection_watermark: format!("rev-{}", version.revision),
        generated_at: Some(now_timestamp()),
        generator_version: version.generator_version.clone(),
    }
}

fn proto_bundle_version(metadata: &BundleMetadata) -> BundleVersion {
    BundleVersion {
        revision: metadata.revision,
        content_hash: metadata.content_hash.clone(),
        schema_version: "v1alpha1".to_string(),
        projection_watermark: format!("rev-{}", metadata.revision),
        generated_at: Some(now_timestamp()),
        generator_version: metadata.generator_version.clone(),
    }
}

fn proto_bundle_snapshot(bundle: &RehydrationBundle) -> BundleSnapshot {
    BundleSnapshot {
        snapshot_id: format!(
            "snapshot:{}:{}",
            bundle.case_id().as_str(),
            bundle.role().as_str()
        ),
        case_id: bundle.case_id().as_str().to_string(),
        role: bundle.role().as_str().to_string(),
        bundle: Some(proto_bundle_from_single_role(bundle)),
        created_at: Some(now_timestamp()),
        expires_at: Some(now_plus_timestamp(900)),
        ttl: Some(proto_duration(900)),
    }
}

fn proto_projection_status(consumer_name: &str) -> ProjectionStatus {
    ProjectionStatus {
        consumer_name: consumer_name.to_string(),
        stream_name: format!("{consumer_name}.events"),
        projection_watermark: "rev-0".to_string(),
        processed_events: 0,
        pending_events: 0,
        last_event_at: Some(now_timestamp()),
        updated_at: Some(now_timestamp()),
        healthy: true,
        warnings: vec!["projection status is placeholder-backed".to_string()],
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
    }
}

fn now_timestamp() -> Timestamp {
    Timestamp::from(SystemTime::now())
}

fn now_plus_timestamp(seconds: u64) -> Timestamp {
    let now = SystemTime::now();
    let shifted = now.checked_add(Duration::from_secs(seconds)).unwrap_or(now);
    Timestamp::from(shifted)
}

fn proto_duration(seconds: u64) -> ProtoDuration {
    ProtoDuration {
        seconds: seconds as i64,
        nanos: 0,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rehydration_domain::{CaseId, RehydrationBundle, Role};
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

    use super::{AdminGrpcService, CommandGrpcService, GrpcServer, QueryGrpcService};

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

    struct NoopSnapshotStore;

    impl SnapshotStore for NoopSnapshotStore {
        fn save_bundle(&self, _bundle: &RehydrationBundle) -> Result<(), PortError> {
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
        let service = QueryGrpcService::new(
            Arc::new(EmptyProjectionReader),
            Arc::new(NoopSnapshotStore),
            "0.1.0",
        );

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
        let service = QueryGrpcService::new(
            Arc::new(EmptyProjectionReader),
            Arc::new(NoopSnapshotStore),
            "0.1.0",
        );

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
        let service = CommandGrpcService::new(Arc::new(
            rehydration_application::UpdateContextUseCase::new("0.1.0"),
        ));

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
    }

    #[tokio::test]
    async fn admin_service_returns_snapshot_and_status() {
        let service = AdminGrpcService::new(Arc::new(EmptyProjectionReader), "0.1.0");

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
        let service = AdminGrpcService::new(Arc::new(EmptyProjectionReader), "0.1.0");

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
}
