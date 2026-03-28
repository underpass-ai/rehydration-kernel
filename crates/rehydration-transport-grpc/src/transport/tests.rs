use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use rehydration_application::{
    AcceptedVersion, ApplicationError, CommandApplicationService, GetContextQuery,
    QueryApplicationService, RehydrateSessionResult,
};
use rehydration_domain::{
    BundleMetadata, BundleNode, BundleNodeDetail, BundleRelationship, CaseId,
    ContextPathNeighborhood, GraphNeighborhoodReader, NodeDetailProjection, NodeDetailReader,
    NodeNeighborhood, NodeProjection, NodeRelationProjection, PortError, RehydrationBundle,
    RelationExplanation, RelationSemanticClass, Role, SnapshotSaveOptions, SnapshotStore,
};
use rehydration_proto::v1beta1::{
    ContextChange, ContextChangeOperation, GetContextPathRequest,
    GetContextRequest, GetNodeDetailRequest, ResolutionTier, UpdateContextRequest,
    ValidateScopeRequest, context_command_service_server::ContextCommandService,
    context_query_service_server::ContextQueryService,
};
use rehydration_observability::quality_observers::NoopQualityObserver;
use rehydration_testkit::InMemoryContextEventStore;
use tokio::sync::Mutex;
use tonic::Request;

use super::command_grpc_service_v1beta1::CommandGrpcServiceV1Beta1;
use super::grpc_server::GrpcServer;
use super::proto_mapping_v1beta1::{
    proto_accepted_version_v1beta1, proto_bundle_from_single_role_v1beta1,
    proto_bundle_node_detail_v1beta1, proto_bundle_node_v1beta1, proto_bundle_relationship_v1beta1,
    proto_rehydrate_session_response_v1beta1,
};
use super::query_grpc_service_v1beta1::QueryGrpcServiceV1Beta1;
use super::support::{map_application_error, proto_duration, trim_to_option};

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

    async fn load_node_details_batch(
        &self,
        node_ids: Vec<String>,
    ) -> Result<Vec<Option<NodeDetailProjection>>, PortError> {
        let mut results = Vec::with_capacity(node_ids.len());
        for node_id in &node_ids {
            results.push(self.load_node_detail(node_id).await?);
        }
        Ok(results)
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
                            provenance: None,
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
                            provenance: None,
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

    async fn load_node_details_batch(
        &self,
        node_ids: Vec<String>,
    ) -> Result<Vec<Option<NodeDetailProjection>>, PortError> {
        let mut results = Vec::with_capacity(node_ids.len());
        for node_id in &node_ids {
            results.push(self.load_node_detail(node_id).await?);
        }
        Ok(results)
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
        InMemoryContextEventStore::new(),
        Arc::new(NoopQualityObserver),
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
        InMemoryContextEventStore::new(),
        Arc::new(NoopQualityObserver),
    );

    let get_context_err = server
        .query_application()
        .get_context(GetContextQuery {
            root_node_id: "node-123".to_string(),
            role: "developer".to_string(),
            depth: 0,
            requested_scopes: vec!["graph".to_string()],
            render_options: Default::default(),
        })
        .await;
    assert!(
        get_context_err.is_err(),
        "empty graph should return NotFound"
    );

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
        })
        .await
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
    let service = QueryGrpcServiceV1Beta1::new(application, Arc::new(NoopQualityObserver));

    let status = service
        .get_context(Request::new(GetContextRequest {
            root_node_id: "node-123".to_string(),
            role: "developer".to_string(),
            token_budget: 1024,
            requested_scopes: vec!["graph".to_string()],
            depth: 0,
            max_tier: 0,
            rehydration_mode: 0,
        }))
        .await
        .expect_err("empty graph should return NOT_FOUND");
    assert_eq!(status.code(), tonic::Code::NotFound);
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
    let service = QueryGrpcServiceV1Beta1::new(application, Arc::new(NoopQualityObserver));

    let _ = service
        .get_context(Request::new(GetContextRequest {
            root_node_id: "node-123".to_string(),
            role: "developer".to_string(),
            token_budget: 1024,
            requested_scopes: vec!["graph".to_string()],
            depth: 17,
            max_tier: 0,
            rehydration_mode: 0,
        }))
        .await;

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
    let service = QueryGrpcServiceV1Beta1::new(application, Arc::new(NoopQualityObserver));

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
async fn query_service_returns_multi_resolution_tiers_in_rendered_context() {
    let application = Arc::new(QueryApplicationService::new(
        Arc::new(SeededGraphNeighborhoodReader),
        Arc::new(SeededNodeDetailReader),
        Arc::new(NoopSnapshotStore),
        "0.1.0",
    ));
    let service = QueryGrpcServiceV1Beta1::new(application, Arc::new(NoopQualityObserver));

    let response = service
        .get_context_path(Request::new(GetContextPathRequest {
            root_node_id: "node-123".to_string(),
            target_node_id: "node-789".to_string(),
            role: "developer".to_string(),
            token_budget: 4096,
        }))
        .await
        .expect("should succeed")
        .into_inner();

    let rendered = response.rendered.expect("rendered should exist");

    // Flat content still works (backward compat)
    assert!(!rendered.content.is_empty());
    assert!(rendered.token_count > 0);
    assert!(!rendered.sections.is_empty());

    // Tiers are populated
    assert!(
        !rendered.tiers.is_empty(),
        "tiers should be populated in the gRPC response"
    );

    // L0 Summary is always first
    let l0 = &rendered.tiers[0];
    assert_eq!(l0.tier, ResolutionTier::L0Summary as i32);
    assert!(l0.content.contains("Objective:"));
    assert!(l0.token_count > 0);

    // At least L0 + L1 present
    assert!(
        rendered.tiers.len() >= 2,
        "should have at least L0 and L1, got {}",
        rendered.tiers.len()
    );
}

#[tokio::test]
async fn query_service_returns_node_detail_panel() {
    let application = Arc::new(QueryApplicationService::new(
        Arc::new(SeededGraphNeighborhoodReader),
        Arc::new(SeededNodeDetailReader),
        Arc::new(NoopSnapshotStore),
        "0.1.0",
    ));
    let service = QueryGrpcServiceV1Beta1::new(application, Arc::new(NoopQualityObserver));

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
    let service = QueryGrpcServiceV1Beta1::new(application, Arc::new(NoopQualityObserver));

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
    let service = QueryGrpcServiceV1Beta1::new(application, Arc::new(NoopQualityObserver));

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
    let service = QueryGrpcServiceV1Beta1::new(application, Arc::new(NoopQualityObserver));

    let response = service
        .validate_scope(Request::new(ValidateScopeRequest {
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
    let event_store = Arc::new(InMemoryContextEventStore::new());
    let service =
        CommandGrpcServiceV1Beta1::new(Arc::new(CommandApplicationService::new(Arc::new(
            rehydration_application::UpdateContextUseCase::new(event_store, "0.1.0"),
        ))));

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
}

#[tokio::test]
async fn command_service_returns_aborted_on_revision_conflict() {
    let event_store = Arc::new(InMemoryContextEventStore::new());
    let service =
        CommandGrpcServiceV1Beta1::new(Arc::new(CommandApplicationService::new(Arc::new(
            rehydration_application::UpdateContextUseCase::new(event_store, "0.1.0"),
        ))));

    let status = service
        .update_context(Request::new(UpdateContextRequest {
            root_node_id: "node-123".to_string(),
            role: "developer".to_string(),
            work_item_id: String::new(),
            changes: vec![],
            metadata: None,
            precondition: Some(rehydration_proto::v1beta1::RevisionPrecondition {
                expected_revision: 99,
                expected_content_hash: String::new(),
            }),
        }))
        .await
        .expect_err("wrong revision should fail");

    assert_eq!(status.code(), tonic::Code::Aborted);
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
    let response = proto_rehydrate_session_response_v1beta1(&RehydrateSessionResult {
        root_node_id: "node-123".to_string(),
        bundles: vec![sample_bundle("node-123", "developer", "Section one")],
        timeline_events: 9,
        version: BundleMetadata::initial("0.1.0"),
        snapshot_persisted: true,
        snapshot_id: Some("snapshot:node-123:developer".to_string()),
        generated_at: now,
        timing: None,
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
            provenance: None,
        },
        neighbors: Vec::new(),
        relations: Vec::new(),
    }
}
