use std::sync::Arc;
use std::time::{Duration, SystemTime};

use rehydration_application::{
    AcceptedVersion, AdminCommandApplicationService, AdminQueryApplicationService,
    ApplicationError, BundleAssembler, BundleSnapshotResult, CommandApplicationService,
    GetGraphRelationshipsResult, GetProjectionStatusResult, GetRehydrationDiagnosticsResult,
    GraphNodeView, GraphRelationshipView, ProjectionStatusView, QueryApplicationService,
    RehydrateSessionResult, RehydrationDiagnosticView, ReplayModeSelection,
    ReplayProjectionOutcome,
};
use rehydration_domain::{
    BundleMetadata, CaseHeader, CaseId, Decision, GraphNeighborhoodReader, Milestone,
    NodeDetailProjection, NodeDetailReader, PlanHeader, PortError, RehydrationBundle, Role,
    RoleContextPack, SnapshotStore, TaskImpact, WorkItem,
};
use rehydration_proto::v1alpha1::{
    BundleRenderFormat, ContextChange, ContextChangeOperation, GetBundleSnapshotRequest,
    GetContextRequest, GetProjectionStatusRequest, GetRehydrationDiagnosticsRequest, Phase,
    UpdateContextRequest, ValidateScopeRequest, context_admin_service_server::ContextAdminService,
    context_command_service_server::ContextCommandService,
    context_query_service_server::ContextQueryService,
};
use tonic::Request;

use super::admin_grpc_service::AdminGrpcService;
use super::command_grpc_service::CommandGrpcService;
use super::grpc_server::GrpcServer;
use super::proto_mapping::{
    proto_accepted_version, proto_bundle_snapshot_response, proto_bundle_version,
    proto_graph_relationships_response, proto_projection_status_response,
    proto_rehydrate_session_response, proto_rehydration_diagnostics_response,
    proto_replay_projection_response, proto_role_pack_from_domain,
};
use super::query_grpc_service::QueryGrpcService;
use super::support::{
    map_application_error, map_replay_mode, proto_duration, proto_replay_mode, trim_to_option,
};

struct EmptyGraphNeighborhoodReader;

impl GraphNeighborhoodReader for EmptyGraphNeighborhoodReader {
    async fn load_neighborhood(
        &self,
        _root_node_id: &str,
    ) -> Result<Option<rehydration_domain::NodeNeighborhood>, PortError> {
        Ok(None)
    }
}

struct EmptyNodeDetailReader;

impl NodeDetailReader for EmptyNodeDetailReader {
    async fn load_node_detail(
        &self,
        _node_id: &str,
    ) -> Result<Option<NodeDetailProjection>, PortError> {
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
        detail_uri: "redis://localhost:6379".to_string(),
        snapshot_uri: "redis://localhost:6379".to_string(),
        events_subject_prefix: "rehydration".to_string(),
    };
    let server = GrpcServer::new(
        config,
        EmptyGraphNeighborhoodReader,
        EmptyNodeDetailReader,
        NoopSnapshotStore,
    );

    assert!(server.describe().contains("127.0.0.1:50054"));
    assert_eq!(server.bootstrap_request().root_node_id, "bootstrap-case");
    let descriptor_set = std::hint::black_box(server.descriptor_set());
    assert!(!descriptor_set.is_empty());
}

#[tokio::test]
async fn query_service_returns_rendered_context() {
    let application = Arc::new(QueryApplicationService::new(
        Arc::new(EmptyGraphNeighborhoodReader),
        Arc::new(EmptyNodeDetailReader),
        Arc::new(NoopSnapshotStore),
        "0.1.0",
    ));
    let service = QueryGrpcService::new(application);

    let response = service
        .get_context(Request::new(GetContextRequest {
            root_node_id: "case-123".to_string(),
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
            .root_node_id,
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
        Arc::new(EmptyGraphNeighborhoodReader),
        Arc::new(EmptyNodeDetailReader),
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
            root_node_id: "case-123".to_string(),
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
    let application = Arc::new(AdminQueryApplicationService::new(
        Arc::new(EmptyGraphNeighborhoodReader),
        Arc::new(EmptyNodeDetailReader),
        "0.1.0",
    ));
    let service = AdminGrpcService::new(application, Arc::new(AdminCommandApplicationService));

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
            root_node_id: "case-123".to_string(),
            role: "developer".to_string(),
        }))
        .await
        .expect("bundle snapshot should succeed")
        .into_inner()
        .snapshot
        .expect("snapshot should exist");
    assert_eq!(snapshot.root_node_id, "case-123");
    assert_eq!(snapshot.role, "developer");
}

#[tokio::test]
async fn admin_service_returns_diagnostics_per_role() {
    let application = Arc::new(AdminQueryApplicationService::new(
        Arc::new(EmptyGraphNeighborhoodReader),
        Arc::new(EmptyNodeDetailReader),
        "0.1.0",
    ));
    let service = AdminGrpcService::new(application, Arc::new(AdminCommandApplicationService));

    let response = service
        .get_rehydration_diagnostics(Request::new(GetRehydrationDiagnosticsRequest {
            root_node_id: "case-123".to_string(),
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
    let bundle = RehydrationBundle::new(
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
        vec!["Projection-backed summary".to_string()],
        BundleMetadata::initial("0.1.0"),
    );

    let proto = proto_role_pack_from_domain(&bundle);

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

    let diagnostics = proto_rehydration_diagnostics_response(&GetRehydrationDiagnosticsResult {
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
        root_node_id: "case-123".to_string(),
        bundles: vec![RehydrationBundle::new(
            RoleContextPack::new(
                Role::new("developer").expect("role is valid"),
                CaseHeader::new(
                    CaseId::new("case-123").expect("case id is valid"),
                    "Case 123",
                    "Section one",
                    "ACTIVE",
                    SystemTime::UNIX_EPOCH,
                    "test",
                ),
                None,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                "Section one",
                4096,
            ),
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
        root_node_id: "case-123".to_string(),
        role: "developer".to_string(),
        bundle: BundleAssembler::placeholder("case-123", "developer", "0.1.0")
            .expect("placeholder bundle should build"),
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
