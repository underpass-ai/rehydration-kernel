use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use rehydration_application::{
    AcceptedVersion, AdminCommandApplicationService, AdminQueryApplicationService,
    ApplicationError, BundleAssembler, BundleSnapshotResult, CommandApplicationService,
    GetContextQuery, GetGraphRelationshipsResult, GetProjectionStatusResult,
    GetRehydrationDiagnosticsResult, GraphNodeView, GraphRelationshipView, ProjectionStatusView,
    QueryApplicationService, RehydrateSessionResult, RehydrationDiagnosticView,
    ReplayModeSelection, ReplayProjectionOutcome,
};
use rehydration_domain::{
    BundleMetadata, BundleNode, BundleNodeDetail, BundleRelationship, CaseId,
    ContextPathNeighborhood, GraphNeighborhoodReader, NodeDetailProjection, NodeDetailReader,
    NodeNeighborhood, NodeProjection, NodeRelationProjection, PortError, RehydrationBundle,
    RelationExplanation, RelationSemanticClass, Role, SnapshotSaveOptions, SnapshotStore,
};
use rehydration_proto::fleet_context_v1::{
    ContextChange as CompatibilityContextChange,
    CreateStoryRequest as CompatibilityCreateStoryRequest,
    GetContextRequest as CompatibilityGetContextRequest,
    GetGraphRelationshipsRequest as CompatibilityGetGraphRelationshipsRequest,
    RehydrateSessionRequest as CompatibilityRehydrateSessionRequest,
    UpdateContextRequest as CompatibilityUpdateContextRequest,
    ValidateScopeRequest as CompatibilityValidateScopeRequest,
    context_service_server::ContextService,
};
use rehydration_proto::v1beta1::{
    BundleRenderFormat, ContextChange, ContextChangeOperation, GetBundleSnapshotRequest,
    GetContextPathRequest, GetContextRequest, GetNodeDetailRequest, GetProjectionStatusRequest,
    GetRehydrationDiagnosticsRequest, Phase, UpdateContextRequest, ValidateScopeRequest,
    context_admin_service_server::ContextAdminService,
    context_command_service_server::ContextCommandService,
    context_query_service_server::ContextQueryService,
};
use tokio::sync::Mutex;
use tonic::Request;

use super::admin_grpc_service_v1beta1::AdminGrpcServiceV1Beta1;
use super::command_grpc_service_v1beta1::CommandGrpcServiceV1Beta1;
use super::context_service_compatibility::ContextCompatibilityGrpcService;
use super::grpc_server::GrpcServer;
use super::proto_mapping_v1beta1::{
    proto_accepted_version_v1beta1, proto_bundle_from_single_role_v1beta1,
    proto_bundle_node_detail_v1beta1, proto_bundle_node_v1beta1, proto_bundle_relationship_v1beta1,
    proto_bundle_snapshot_response_v1beta1, proto_graph_node_v1beta1,
    proto_graph_relationships_response_v1beta1, proto_projection_status_response_v1beta1,
    proto_rehydrate_session_response_v1beta1, proto_rehydration_diagnostics_response_v1beta1,
    proto_replay_projection_response_v1beta1,
};
use super::query_grpc_service_v1beta1::QueryGrpcServiceV1Beta1;
use super::support::{
    map_application_error, map_replay_mode_v1beta1, proto_duration, proto_replay_mode_v1beta1,
    trim_to_option,
};

struct EmptyGraphNeighborhoodReader;

impl GraphNeighborhoodReader for EmptyGraphNeighborhoodReader {
    async fn load_neighborhood(
        &self,
        _root_node_id: &str,
        _depth: u32,
    ) -> Result<Option<NodeNeighborhood>, PortError> {
        Ok(None)
    }

    async fn load_context_path(
        &self,
        _root_node_id: &str,
        _target_node_id: &str,
        _subtree_depth: u32,
    ) -> Result<Option<ContextPathNeighborhood>, PortError> {
        Ok(None)
    }
}

struct RecordingGraphNeighborhoodReader {
    depths: Arc<Mutex<Vec<u32>>>,
}

impl GraphNeighborhoodReader for RecordingGraphNeighborhoodReader {
    async fn load_neighborhood(
        &self,
        _root_node_id: &str,
        depth: u32,
    ) -> Result<Option<NodeNeighborhood>, PortError> {
        self.depths.lock().await.push(depth);
        Ok(None)
    }

    async fn load_context_path(
        &self,
        _root_node_id: &str,
        _target_node_id: &str,
        subtree_depth: u32,
    ) -> Result<Option<ContextPathNeighborhood>, PortError> {
        self.depths.lock().await.push(subtree_depth);
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

struct SeededGraphNeighborhoodReader;

impl GraphNeighborhoodReader for SeededGraphNeighborhoodReader {
    async fn load_neighborhood(
        &self,
        root_node_id: &str,
        _depth: u32,
    ) -> Result<Option<NodeNeighborhood>, PortError> {
        match root_node_id {
            "node-123" => Ok(Some(sample_node_neighborhood("node-123", "ACTIVE"))),
            "graph-only" => Ok(Some(sample_node_neighborhood("graph-only", "READY"))),
            _ => Ok(None),
        }
    }

    async fn load_context_path(
        &self,
        root_node_id: &str,
        target_node_id: &str,
        _subtree_depth: u32,
    ) -> Result<Option<ContextPathNeighborhood>, PortError> {
        Ok(
            (root_node_id == "node-123" && target_node_id == "node-789").then_some(
                ContextPathNeighborhood {
                    root: sample_node_neighborhood("node-123", "ACTIVE").root,
                    neighbors: vec![
                        NodeProjection {
                            node_id: "node-456".to_string(),
                            node_kind: "task".to_string(),
                            title: "Node node-456".to_string(),
                            summary: "Summary for node-456".to_string(),
                            status: "OPEN".to_string(),
                            labels: vec!["Task".to_string()],
                            properties: [("owner".to_string(), "ops".to_string())]
                                .into_iter()
                                .collect(),
                        },
                        NodeProjection {
                            node_id: "node-789".to_string(),
                            node_kind: "task".to_string(),
                            title: "Node node-789".to_string(),
                            summary: "Summary for node-789".to_string(),
                            status: "READY".to_string(),
                            labels: vec!["Task".to_string()],
                            properties: [("owner".to_string(), "ops".to_string())]
                                .into_iter()
                                .collect(),
                        },
                    ],
                    relations: vec![
                        NodeRelationProjection {
                            source_node_id: "node-123".to_string(),
                            target_node_id: "node-456".to_string(),
                            relation_type: "HAS_TASK".to_string(),
                            explanation: RelationExplanation::new(
                                RelationSemanticClass::Structural,
                            )
                            .with_sequence(1),
                        },
                        NodeRelationProjection {
                            source_node_id: "node-456".to_string(),
                            target_node_id: "node-789".to_string(),
                            relation_type: "HAS_TASK".to_string(),
                            explanation: RelationExplanation::new(
                                RelationSemanticClass::Structural,
                            )
                            .with_sequence(2),
                        },
                    ],
                    path_node_ids: vec![
                        "node-123".to_string(),
                        "node-456".to_string(),
                        "node-789".to_string(),
                    ],
                },
            ),
        )
    }
}

struct SeededNodeDetailReader;

impl NodeDetailReader for SeededNodeDetailReader {
    async fn load_node_detail(
        &self,
        node_id: &str,
    ) -> Result<Option<NodeDetailProjection>, PortError> {
        Ok(match node_id {
            "node-123" => Some(NodeDetailProjection {
                node_id: "node-123".to_string(),
                detail: "Expanded detail".to_string(),
                content_hash: "hash-1".to_string(),
                revision: 2,
            }),
            "node-456" => Some(NodeDetailProjection {
                node_id: "node-456".to_string(),
                detail: "Middle detail".to_string(),
                content_hash: "hash-456".to_string(),
                revision: 3,
            }),
            "node-789" => Some(NodeDetailProjection {
                node_id: "node-789".to_string(),
                detail: "Target detail".to_string(),
                content_hash: "hash-789".to_string(),
                revision: 4,
            }),
            "orphan-detail" => Some(NodeDetailProjection {
                node_id: "orphan-detail".to_string(),
                detail: "orphaned".to_string(),
                content_hash: "hash-orphan".to_string(),
                revision: 1,
            }),
            _ => None,
        })
    }
}

struct NoopSnapshotStore;

impl SnapshotStore for NoopSnapshotStore {
    async fn save_bundle_with_options(
        &self,
        _bundle: &RehydrationBundle,
        _options: SnapshotSaveOptions,
    ) -> Result<(), PortError> {
        Ok(())
    }
}

#[test]
fn describe_mentions_bind_address() {
    let config = rehydration_config::AppConfig {
        service_name: "rehydration-kernel".to_string(),
        grpc_bind: "127.0.0.1:50054".to_string(),
        admin_bind: "127.0.0.1:8080".to_string(),
        grpc_tls: rehydration_config::GrpcTlsConfig::disabled(),
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
    assert_eq!(server.bootstrap_request().root_node_id, "bootstrap-node");
    let descriptor_set = std::hint::black_box(server.descriptor_set());
    assert!(!descriptor_set.is_empty());
}

#[tokio::test]
async fn grpc_server_application_accessors_return_callable_services() {
    let config = rehydration_config::AppConfig {
        service_name: "rehydration-kernel".to_string(),
        grpc_bind: "127.0.0.1:50054".to_string(),
        admin_bind: "127.0.0.1:8080".to_string(),
        grpc_tls: rehydration_config::GrpcTlsConfig::disabled(),
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

    let get_context = server
        .query_application()
        .get_context(GetContextQuery {
            root_node_id: "node-123".to_string(),
            role: "developer".to_string(),
            depth: 0,
            requested_scopes: vec!["graph".to_string()],
            render_options: Default::default(),
        })
        .await
        .expect("query application should respond");
    assert_eq!(get_context.bundle.root_node_id().as_str(), "node-123");

    let update = server
        .command_application()
        .update_context(rehydration_application::UpdateContextCommand {
            root_node_id: "node-123".to_string(),
            role: "developer".to_string(),
            work_item_id: String::new(),
            changes: Vec::new(),
            expected_revision: None,
            expected_content_hash: None,
            idempotency_key: None,
            requested_by: None,
            persist_snapshot: false,
        })
        .expect("command application should respond");
    assert_eq!(update.accepted_version.revision, 1);
}

#[tokio::test]
async fn query_service_returns_rendered_context() {
    let application = Arc::new(QueryApplicationService::new(
        Arc::new(EmptyGraphNeighborhoodReader),
        Arc::new(EmptyNodeDetailReader),
        Arc::new(NoopSnapshotStore),
        "0.1.0",
    ));
    let service = QueryGrpcServiceV1Beta1::new(application);

    let response = service
        .get_context(Request::new(GetContextRequest {
            root_node_id: "node-123".to_string(),
            role: "developer".to_string(),
            phase: Phase::Build as i32,
            work_item_id: String::new(),
            token_budget: 1024,
            requested_scopes: vec!["graph".to_string()],
            render_format: BundleRenderFormat::Structured as i32,
            include_debug_sections: false,
            depth: 0,
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
        "node-123"
    );
    assert!(
        response
            .rendered
            .as_ref()
            .expect("rendered context should exist")
            .content
            .contains("node-123")
    );
}

#[tokio::test]
async fn query_service_forwards_requested_depth_to_application() {
    let depths = Arc::new(Mutex::new(Vec::new()));
    let application = Arc::new(QueryApplicationService::new(
        Arc::new(RecordingGraphNeighborhoodReader {
            depths: Arc::clone(&depths),
        }),
        Arc::new(EmptyNodeDetailReader),
        Arc::new(NoopSnapshotStore),
        "0.1.0",
    ));
    let service = QueryGrpcServiceV1Beta1::new(application);

    service
        .get_context(Request::new(GetContextRequest {
            root_node_id: "node-123".to_string(),
            role: "developer".to_string(),
            phase: Phase::Build as i32,
            work_item_id: String::new(),
            token_budget: 1024,
            requested_scopes: vec!["graph".to_string()],
            render_format: BundleRenderFormat::Structured as i32,
            include_debug_sections: false,
            depth: 17,
        }))
        .await
        .expect("get context should succeed");

    assert_eq!(&*depths.lock().await, &[17]);
}

#[tokio::test]
async fn query_service_returns_context_path_bundle() {
    let application = Arc::new(QueryApplicationService::new(
        Arc::new(SeededGraphNeighborhoodReader),
        Arc::new(SeededNodeDetailReader),
        Arc::new(NoopSnapshotStore),
        "0.1.0",
    ));
    let service = QueryGrpcServiceV1Beta1::new(application);

    let response = service
        .get_context_path(Request::new(GetContextPathRequest {
            root_node_id: "node-123".to_string(),
            target_node_id: "node-789".to_string(),
            role: "developer".to_string(),
            token_budget: 1024,
        }))
        .await
        .expect("get context path should succeed")
        .into_inner();

    let bundle = response.path_bundle.expect("path bundle should exist");
    let role_bundle = bundle
        .bundles
        .first()
        .expect("path bundle should contain one role bundle");

    assert_eq!(bundle.root_node_id, "node-123");
    assert_eq!(role_bundle.neighbor_nodes.len(), 2);
    assert_eq!(role_bundle.relationships.len(), 2);
    assert_eq!(role_bundle.node_details.len(), 3);
    assert!(
        response
            .rendered
            .as_ref()
            .expect("rendered context should exist")
            .content
            .contains("Target detail")
    );
    assert!(response.served_at.is_some());
}

#[tokio::test]
async fn query_service_returns_node_detail_panel() {
    let application = Arc::new(QueryApplicationService::new(
        Arc::new(SeededGraphNeighborhoodReader),
        Arc::new(SeededNodeDetailReader),
        Arc::new(NoopSnapshotStore),
        "0.1.0",
    ));
    let service = QueryGrpcServiceV1Beta1::new(application);

    let response = service
        .get_node_detail(Request::new(GetNodeDetailRequest {
            node_id: "node-123".to_string(),
        }))
        .await
        .expect("get node detail should succeed")
        .into_inner();

    let node = response.node.expect("node should exist");
    let detail = response.detail.expect("detail should exist");

    assert_eq!(node.node_id, "node-123");
    assert_eq!(node.title, "Node node-123");
    assert_eq!(node.properties["owner"], "ops");
    assert_eq!(detail.detail, "Expanded detail");
    assert_eq!(detail.revision, 2);
}

#[tokio::test]
async fn query_service_returns_node_metadata_when_detail_is_missing() {
    let application = Arc::new(QueryApplicationService::new(
        Arc::new(SeededGraphNeighborhoodReader),
        Arc::new(EmptyNodeDetailReader),
        Arc::new(NoopSnapshotStore),
        "0.1.0",
    ));
    let service = QueryGrpcServiceV1Beta1::new(application);

    let response = service
        .get_node_detail(Request::new(GetNodeDetailRequest {
            node_id: "graph-only".to_string(),
        }))
        .await
        .expect("graph-only node detail should succeed")
        .into_inner();

    assert_eq!(
        response.node.expect("node should exist").node_id,
        "graph-only"
    );
    assert!(response.detail.is_none());
}

#[tokio::test]
async fn query_service_returns_not_found_for_missing_node_detail_target() {
    let application = Arc::new(QueryApplicationService::new(
        Arc::new(SeededGraphNeighborhoodReader),
        Arc::new(SeededNodeDetailReader),
        Arc::new(NoopSnapshotStore),
        "0.1.0",
    ));
    let service = QueryGrpcServiceV1Beta1::new(application);

    let error = service
        .get_node_detail(Request::new(GetNodeDetailRequest {
            node_id: "orphan-detail".to_string(),
        }))
        .await
        .expect_err("orphan detail should map to not found");

    assert_eq!(error.code(), tonic::Code::NotFound);
    assert_eq!(error.message(), "Node not found: orphan-detail");
}

#[tokio::test]
async fn query_service_validates_scope_diffs() {
    let application = Arc::new(QueryApplicationService::new(
        Arc::new(EmptyGraphNeighborhoodReader),
        Arc::new(EmptyNodeDetailReader),
        Arc::new(NoopSnapshotStore),
        "0.1.0",
    ));
    let service = QueryGrpcServiceV1Beta1::new(application);

    let response = service
        .validate_scope(Request::new(ValidateScopeRequest {
            role: "developer".to_string(),
            phase: Phase::Build as i32,
            required_scopes: vec!["graph".to_string()],
            provided_scopes: vec!["details".to_string()],
        }))
        .await
        .expect("validate scope should succeed")
        .into_inner();

    let result = response.result.expect("validation result should exist");
    assert!(!result.allowed);
    assert_eq!(result.missing_scopes, vec!["graph".to_string()]);
}

#[tokio::test]
async fn command_service_accepts_update_context() {
    let service = CommandGrpcServiceV1Beta1::new(Arc::new(CommandApplicationService::new(
        Arc::new(rehydration_application::UpdateContextUseCase::new("0.1.0")),
    )));

    let response = service
        .update_context(Request::new(UpdateContextRequest {
            root_node_id: "node-123".to_string(),
            role: "developer".to_string(),
            work_item_id: String::new(),
            changes: vec![ContextChange {
                operation: ContextChangeOperation::Update as i32,
                entity_kind: "node_detail".to_string(),
                entity_id: "node-456".to_string(),
                payload_json: "{\"status\":\"ACTIVE\"}".to_string(),
                reason: "refined".to_string(),
                scopes: vec!["graph".to_string()],
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
    assert_eq!(response.snapshot_id, "snapshot:node-123:developer");
}

#[tokio::test]
async fn admin_service_returns_snapshot_and_status() {
    let application = Arc::new(AdminQueryApplicationService::new(
        Arc::new(EmptyGraphNeighborhoodReader),
        Arc::new(EmptyNodeDetailReader),
        "0.1.0",
    ));
    let service =
        AdminGrpcServiceV1Beta1::new(application, Arc::new(AdminCommandApplicationService));

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
            root_node_id: "node-123".to_string(),
            role: "developer".to_string(),
        }))
        .await
        .expect("bundle snapshot should succeed")
        .into_inner()
        .snapshot
        .expect("snapshot should exist");
    assert_eq!(snapshot.root_node_id, "node-123");
    assert_eq!(snapshot.role, "developer");
}

#[tokio::test]
async fn admin_service_returns_diagnostics_per_role() {
    let application = Arc::new(AdminQueryApplicationService::new(
        Arc::new(EmptyGraphNeighborhoodReader),
        Arc::new(EmptyNodeDetailReader),
        "0.1.0",
    ));
    let service =
        AdminGrpcServiceV1Beta1::new(application, Arc::new(AdminCommandApplicationService));

    let response = service
        .get_rehydration_diagnostics(Request::new(GetRehydrationDiagnosticsRequest {
            root_node_id: "node-123".to_string(),
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
        "v1beta1"
    );
}

#[tokio::test]
async fn compatibility_service_returns_external_context_shape() {
    let service = compatibility_service();

    let response = service
        .get_context(Request::new(CompatibilityGetContextRequest {
            story_id: "node-123".to_string(),
            role: "developer".to_string(),
            phase: "BUILD".to_string(),
            subtask_id: String::new(),
            token_budget: 1024,
        }))
        .await
        .expect("compatibility get context should succeed")
        .into_inner();

    assert!(response.context.contains("node-123"));
    assert_eq!(response.version, "rev-1");
    assert_eq!(
        response.scopes,
        vec![
            "CASE_HEADER".to_string(),
            "DECISIONS_RELEVANT_ROLE".to_string(),
            "DEPS_RELEVANT".to_string(),
            "PLAN_HEADER".to_string(),
            "SUBTASKS_ROLE".to_string(),
        ]
    );
    let blocks = response.blocks.expect("blocks");
    assert_eq!(blocks.system, "role=developer");
    assert!(blocks.tools.contains("CASE_HEADER"));
}

#[tokio::test]
async fn compatibility_service_routes_rehydrate_session_and_builds_packs() {
    let service = compatibility_service();

    let response = service
        .rehydrate_session(Request::new(CompatibilityRehydrateSessionRequest {
            case_id: "node-123".to_string(),
            roles: vec!["developer".to_string()],
            include_timeline: true,
            include_summaries: true,
            timeline_events: 5,
            persist_bundle: false,
            ttl_seconds: 900,
        }))
        .await
        .expect("compatibility rehydrate session should succeed")
        .into_inner();

    assert_eq!(response.case_id, "node-123");
    assert!(response.packs.contains_key("developer"));
    assert_eq!(response.stats.expect("stats").events, 5);
}

#[tokio::test]
async fn compatibility_service_routes_update_context() {
    let service = compatibility_service();

    let response = service
        .update_context(Request::new(CompatibilityUpdateContextRequest {
            story_id: "node-123".to_string(),
            task_id: "task-7".to_string(),
            role: "developer".to_string(),
            changes: vec![CompatibilityContextChange {
                operation: "UPDATE".to_string(),
                entity_type: "decision".to_string(),
                entity_id: "decision-9".to_string(),
                payload: "{\"status\":\"accepted\"}".to_string(),
                reason: "refined".to_string(),
            }],
            timestamp: "2026-03-08T10:00:00Z".to_string(),
        }))
        .await
        .expect("compatibility update context should succeed")
        .into_inner();

    assert_eq!(response.version, 1);
    assert!(response.hash.contains("node-123"));
}

#[tokio::test]
async fn compatibility_service_rejects_missing_graph_relationship_root() {
    let service = compatibility_service();

    let error = service
        .get_graph_relationships(Request::new(CompatibilityGetGraphRelationshipsRequest {
            node_id: "node-123".to_string(),
            node_type: "Story".to_string(),
            depth: 7,
        }))
        .await
        .expect_err("compatibility graph relationships should reject missing nodes");

    assert_eq!(error.code(), tonic::Code::InvalidArgument);
    assert_eq!(error.message(), "Node not found: node-123");
}

#[tokio::test]
async fn compatibility_service_validates_scope_via_external_contract() {
    let service = compatibility_service();

    let validate_scope = service
        .validate_scope(Request::new(CompatibilityValidateScopeRequest {
            role: "developer".to_string(),
            phase: "BUILD".to_string(),
            provided_scopes: vec![
                "CASE_HEADER".to_string(),
                "PLAN_HEADER".to_string(),
                "SUBTASKS_ROLE".to_string(),
                "DECISIONS_RELEVANT_ROLE".to_string(),
                "DEPS_RELEVANT".to_string(),
            ],
        }))
        .await
        .expect("validate scope should succeed")
        .into_inner();

    assert!(validate_scope.allowed);
    assert!(validate_scope.missing.is_empty());
    assert!(validate_scope.extra.is_empty());
    assert_eq!(validate_scope.reason, "All scopes are allowed");
}

#[tokio::test]
async fn compatibility_service_returns_unimplemented_for_remaining_placeholder_rpcs() {
    let service = compatibility_service();

    let create_story = service
        .create_story(Request::new(CompatibilityCreateStoryRequest {
            story_id: "story-1".to_string(),
            title: "Story".to_string(),
            description: "Description".to_string(),
            initial_phase: "DESIGN".to_string(),
        }))
        .await
        .expect_err("create story should be unimplemented");
    let create_task = service
        .create_task(Request::new(
            rehydration_proto::fleet_context_v1::CreateTaskRequest {
                story_id: "story-1".to_string(),
                task_id: "task-1".to_string(),
                title: "Task".to_string(),
                description: "Description".to_string(),
                role: "DEV".to_string(),
                dependencies: Vec::new(),
                priority: 1,
                estimated_hours: 4,
            },
        ))
        .await
        .expect_err("create task should be unimplemented");
    let add_decision = service
        .add_project_decision(Request::new(
            rehydration_proto::fleet_context_v1::AddProjectDecisionRequest {
                story_id: "story-1".to_string(),
                decision_type: "ARCHITECTURE".to_string(),
                title: "Decision".to_string(),
                rationale: "Because".to_string(),
                alternatives_considered: String::new(),
                metadata: Default::default(),
            },
        ))
        .await
        .expect_err("add decision should be unimplemented");
    let transition_phase = service
        .transition_phase(Request::new(
            rehydration_proto::fleet_context_v1::TransitionPhaseRequest {
                story_id: "story-1".to_string(),
                from_phase: "DESIGN".to_string(),
                to_phase: "BUILD".to_string(),
                rationale: "Ready".to_string(),
            },
        ))
        .await
        .expect_err("transition phase should be unimplemented");

    assert_eq!(create_story.code(), tonic::Code::Unimplemented);
    assert_eq!(create_task.code(), tonic::Code::Unimplemented);
    assert_eq!(add_decision.code(), tonic::Code::Unimplemented);
    assert_eq!(transition_phase.code(), tonic::Code::Unimplemented);
}

#[test]
fn helper_mappers_cover_bundle_mapping() {
    let bundle = sample_bundle("node-123", "developer", "Projection-backed summary");
    let proto_bundle = proto_bundle_from_single_role_v1beta1(&bundle);
    let proto_root = proto_bundle_node_v1beta1(bundle.root_node());
    let proto_relationship = proto_bundle_relationship_v1beta1(&bundle.relationships()[0]);
    let proto_detail = proto_bundle_node_detail_v1beta1(&bundle.node_details()[0]);

    assert_eq!(proto_bundle.root_node_id, "node-123");
    assert_eq!(proto_bundle.bundles.len(), 1);
    assert_eq!(proto_bundle.bundles[0].role, "developer");
    assert_eq!(
        proto_bundle.bundles[0]
            .root_node
            .as_ref()
            .expect("root node should exist")
            .summary,
        "Projection-backed summary"
    );
    assert_eq!(proto_bundle.bundles[0].neighbor_nodes.len(), 1);
    assert_eq!(proto_bundle.bundles[0].relationships.len(), 1);
    assert_eq!(proto_bundle.bundles[0].node_details.len(), 1);
    assert_eq!(
        proto_bundle
            .stats
            .as_ref()
            .expect("stats should exist")
            .nodes,
        2
    );
    assert_eq!(proto_root.status, "ACTIVE");
    assert_eq!(proto_relationship.relationship_type, "RELATES_TO");
    assert_eq!(proto_detail.content_hash, "hash-1");
}

#[test]
fn helper_mappers_cover_projection_replay_and_diagnostics() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    let projection_view = ProjectionStatusView {
        consumer_name: "context-projection".to_string(),
        stream_name: "graph.node.materialized".to_string(),
        projection_watermark: "evt-42".to_string(),
        processed_events: 12,
        pending_events: 1,
        last_event_at: now,
        updated_at: now,
        healthy: true,
        warnings: vec!["lagging".to_string()],
    };
    let projection_status = proto_projection_status_response_v1beta1(&GetProjectionStatusResult {
        projections: vec![projection_view.clone()],
        observed_at: now,
    });
    assert_eq!(
        projection_status.projections[0].consumer_name,
        "context-projection"
    );
    assert_eq!(
        projection_status.projections[0].stream_name,
        "graph.node.materialized"
    );

    let replay = proto_replay_projection_response_v1beta1(&ReplayProjectionOutcome {
        replay_id: "replay-1".to_string(),
        consumer_name: "context-projection".to_string(),
        replay_mode: ReplayModeSelection::Rebuild,
        accepted_events: 42,
        requested_at: now,
    });
    assert_eq!(
        replay.replay_mode,
        rehydration_proto::v1beta1::ReplayMode::Rebuild as i32
    );

    let diagnostic_view = RehydrationDiagnosticView {
        role: "developer".to_string(),
        version: BundleMetadata::initial("0.1.0"),
        selected_nodes: 2,
        selected_relationships: 1,
        detailed_nodes: 1,
        estimated_tokens: 256,
        notes: vec!["ok".to_string()],
    };
    let diagnostics =
        proto_rehydration_diagnostics_response_v1beta1(&GetRehydrationDiagnosticsResult {
            diagnostics: vec![diagnostic_view.clone()],
            observed_at: now,
        });
    let diagnostic = diagnostics
        .diagnostics
        .first()
        .expect("diagnostic should exist")
        .clone();
    assert_eq!(diagnostics.diagnostics[0].estimated_tokens, 256);
    assert_eq!(diagnostic.selected_relationships, 1);

    let root = GraphNodeView {
        node_id: "root".to_string(),
        node_kind: "capability".to_string(),
        title: "Root".to_string(),
        summary: "Root summary".to_string(),
        status: "ACTIVE".to_string(),
        labels: vec!["Capability".to_string()],
        properties: [("phase".to_string(), "build".to_string())]
            .into_iter()
            .collect(),
    };
    let neighbor = GraphNodeView {
        node_id: "node-1".to_string(),
        node_kind: "artifact".to_string(),
        title: "Neighbor".to_string(),
        summary: "Neighbor summary".to_string(),
        status: "ACTIVE".to_string(),
        labels: vec!["Artifact".to_string()],
        properties: Default::default(),
    };
    let relationship = GraphRelationshipView {
        source_node_id: "root".to_string(),
        target_node_id: "node-1".to_string(),
        relationship_type: "DEPENDS_ON".to_string(),
        explanation: RelationExplanation::new(RelationSemanticClass::Constraint).with_sequence(1),
    };
    let graph = proto_graph_relationships_response_v1beta1(&GetGraphRelationshipsResult {
        root: root.clone(),
        neighbors: vec![neighbor.clone()],
        relationships: vec![relationship.clone()],
        observed_at: now,
    });
    let graph_root = proto_graph_node_v1beta1(&root);
    let graph_relationship = graph
        .relationships
        .first()
        .expect("graph relationship should exist")
        .clone();

    assert_eq!(graph.neighbors.len(), 1);
    assert_eq!(graph.relationships[0].relationship_type, "DEPENDS_ON");
    assert_eq!(graph_root.summary, "Root summary");
    assert_eq!(graph_relationship.target_node_id, "node-1");
}

#[test]
fn helper_mappers_cover_versions_errors_and_trim_logic() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_100);
    let version = super::proto_mapping_v1beta1::proto_bundle_version_v1beta1(&BundleMetadata {
        revision: 7,
        content_hash: "abc123".to_string(),
        generator_version: "0.1.0".to_string(),
    });
    let accepted = proto_accepted_version_v1beta1(&AcceptedVersion {
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
    assert_eq!(
        map_application_error(ApplicationError::NotFound("missing".to_string())).code(),
        tonic::Code::NotFound
    );
    assert_eq!(
        proto_replay_mode_v1beta1(map_replay_mode_v1beta1(999)) as i32,
        1
    );
    assert_eq!(
        proto_replay_mode_v1beta1(ReplayModeSelection::Rebuild) as i32,
        2
    );

    let response = proto_rehydrate_session_response_v1beta1(&RehydrateSessionResult {
        root_node_id: "node-123".to_string(),
        bundles: vec![sample_bundle("node-123", "developer", "Section one")],
        timeline_events: 9,
        version: BundleMetadata::initial("0.1.0"),
        snapshot_persisted: true,
        snapshot_id: Some("snapshot:node-123:developer".to_string()),
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

    let snapshot = proto_bundle_snapshot_response_v1beta1(&BundleSnapshotResult {
        snapshot_id: "snapshot:node-123:developer".to_string(),
        root_node_id: "node-123".to_string(),
        role: "developer".to_string(),
        bundle: BundleAssembler::placeholder("node-123", "developer", "0.1.0")
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
        rehydration_domain::DomainError::EmptyValue("root_node_id"),
    ));
    assert_eq!(invalid_argument.code(), tonic::Code::InvalidArgument);
}

fn sample_bundle(root_node_id: &str, role: &str, summary: &str) -> RehydrationBundle {
    let case_id = CaseId::new(root_node_id).expect("root node id is valid");
    let role = Role::new(role).expect("role is valid");

    RehydrationBundle::new(
        case_id.clone(),
        role,
        BundleNode::new(
            case_id.as_str(),
            "capability",
            format!("Node {}", case_id.as_str()),
            summary,
            "ACTIVE",
            vec!["projection-node".to_string()],
            BTreeMap::new(),
        ),
        vec![BundleNode::new(
            "node-456",
            "artifact",
            "Linked artifact",
            "Linked summary",
            "ACTIVE",
            vec!["artifact".to_string()],
            BTreeMap::new(),
        )],
        vec![BundleRelationship::new(
            case_id.as_str(),
            "node-456",
            "RELATES_TO",
            RelationExplanation::new(RelationSemanticClass::Structural),
        )],
        vec![BundleNodeDetail::new(
            case_id.as_str(),
            summary,
            "hash-1",
            1,
        )],
        BundleMetadata::initial("0.1.0"),
    )
    .expect("sample bundle should be valid")
}

fn sample_node_neighborhood(node_id: &str, status: &str) -> NodeNeighborhood {
    NodeNeighborhood {
        root: rehydration_domain::NodeProjection {
            node_id: node_id.to_string(),
            node_kind: "task".to_string(),
            title: format!("Node {node_id}"),
            summary: format!("Summary for {node_id}"),
            status: status.to_string(),
            labels: vec!["Task".to_string()],
            properties: [("owner".to_string(), "ops".to_string())]
                .into_iter()
                .collect(),
        },
        neighbors: Vec::new(),
        relations: Vec::new(),
    }
}

fn compatibility_service() -> ContextCompatibilityGrpcService<
    EmptyGraphNeighborhoodReader,
    EmptyNodeDetailReader,
    NoopSnapshotStore,
> {
    ContextCompatibilityGrpcService::new(
        Arc::new(QueryApplicationService::new(
            Arc::new(EmptyGraphNeighborhoodReader),
            Arc::new(EmptyNodeDetailReader),
            Arc::new(NoopSnapshotStore),
            "0.1.0",
        )),
        Arc::new(AdminQueryApplicationService::new(
            Arc::new(EmptyGraphNeighborhoodReader),
            Arc::new(EmptyNodeDetailReader),
            "0.1.0",
        )),
        Arc::new(CommandApplicationService::new(Arc::new(
            rehydration_application::UpdateContextUseCase::new("0.1.0"),
        ))),
    )
}
