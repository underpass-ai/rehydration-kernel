#![cfg(feature = "container-tests")]

#[path = "full_tls_support/mod.rs"]
mod agentic_support;

use std::error::Error;
use std::time::Duration as StdDuration;

use agentic_support::kernel_e2e_seed::{
    CHIEF_ENGINEER_TITLE, DECISION_DETAIL, DECISION_ID, DECISION_TITLE,
    EXPECTED_COMPLETED_TASK_COUNT, EXPECTED_DECISION_COUNT, EXPECTED_DECISION_EDGE_COUNT,
    EXPECTED_DETAIL_COUNT, EXPECTED_IMPACT_COUNT, EXPECTED_NEIGHBOR_COUNT,
    EXPECTED_RELATIONSHIP_COUNT, EXPECTED_SELECTED_NODE_COUNT,
    EXPECTED_SELECTED_RELATIONSHIP_COUNT, EXPECTED_TASK_COUNT, EXPECTED_TOKEN_BUDGET_HINT,
    JUMP_DECISION_ID, POWER_TASK_ID, PROPULSION_SUBSYSTEM_TITLE, RELATION_DECISION_REQUIRES,
    RELATION_DEPENDS_ON, RELATION_IMPACTS, ROOT_DETAIL, ROOT_LABEL, ROOT_NODE_ID, ROOT_TITLE,
    TASK_DETAIL, TASK_ID, TASK_TITLE, publish_kernel_e2e_projection_events,
};
use agentic_support::kernel_tls_fixture::KernelTlsFixture;
use agentic_support::seed_data::{
    BUILD_PHASE, DEVELOPER_ROLE, allowed_validate_scope_request_scopes,
    rejected_validate_scope_request_scopes,
};
use prost_types::Duration;
use rehydration_proto::fleet_context_v1::{
    ContextChange as CompatibilityContextChange,
    GetContextRequest as CompatibilityGetContextRequest,
    GetGraphRelationshipsRequest as CompatibilityGetGraphRelationshipsRequest,
    RehydrateSessionRequest as CompatibilityRehydrateSessionRequest,
    UpdateContextRequest as CompatibilityUpdateContextRequest,
    ValidateScopeRequest as CompatibilityValidateScopeRequest,
};
use rehydration_proto::v1alpha1::{
    BundleRenderFormat, CommandMetadata, ContextChange, ContextChangeOperation,
    GetBundleSnapshotRequest, GetContextRequest, GetGraphRelationshipsRequest,
    GetProjectionStatusRequest, GetRehydrationDiagnosticsRequest, Phase, ReplayMode,
    ReplayProjectionRequest, RevisionPrecondition, UpdateContextRequest,
};
use tokio::time::sleep;
use tonic::transport::Channel;

#[tokio::test]
async fn kernel_full_journey_supports_tls_across_transport_surfaces()
-> Result<(), Box<dyn Error + Send + Sync>> {
    let fixture =
        KernelTlsFixture::start_with_seed(ROOT_NODE_ID, TASK_ID, |publisher| async move {
            publish_kernel_e2e_projection_events(&publisher).await
        })
        .await?;

    let result = async {
        let mut compatibility_client = fixture.compatibility_client();
        let mut query_client = fixture.query_client();
        let mut command_client = fixture.command_client();
        let mut admin_client = fixture.admin_client();

        wait_for_full_compatibility_graph(compatibility_client.clone()).await?;

        let compatibility_context = compatibility_client
            .get_context(CompatibilityGetContextRequest {
                story_id: ROOT_NODE_ID.to_string(),
                role: DEVELOPER_ROLE.to_string(),
                phase: BUILD_PHASE.to_string(),
                subtask_id: TASK_ID.to_string(),
                token_budget: 2048,
            })
            .await?
            .into_inner();
        assert!(compatibility_context.context.contains(ROOT_TITLE));
        assert!(compatibility_context.context.contains(TASK_TITLE));
        assert!(compatibility_context.context.contains(TASK_DETAIL));
        assert!(compatibility_context.context.contains(ROOT_DETAIL));
        assert!(compatibility_context.context.contains(DECISION_TITLE));
        assert!(compatibility_context.context.contains(DECISION_DETAIL));
        assert!(
            compatibility_context
                .context
                .contains(PROPULSION_SUBSYSTEM_TITLE)
        );
        assert!(compatibility_context.context.contains(CHIEF_ENGINEER_TITLE));
        assert_eq!(
            compatibility_context
                .blocks
                .as_ref()
                .map(|blocks| blocks.system.as_str()),
            Some("role=developer")
        );

        let allowed_scope = compatibility_client
            .validate_scope(CompatibilityValidateScopeRequest {
                role: DEVELOPER_ROLE.to_string(),
                phase: BUILD_PHASE.to_string(),
                provided_scopes: allowed_validate_scope_request_scopes(),
            })
            .await?
            .into_inner();
        assert!(allowed_scope.allowed);
        assert!(allowed_scope.missing.is_empty());
        assert!(allowed_scope.extra.is_empty());

        let rejected_scope = compatibility_client
            .validate_scope(CompatibilityValidateScopeRequest {
                role: DEVELOPER_ROLE.to_string(),
                phase: BUILD_PHASE.to_string(),
                provided_scopes: rejected_validate_scope_request_scopes(),
            })
            .await?
            .into_inner();
        assert!(!rejected_scope.allowed);
        assert!(
            rejected_scope
                .missing
                .contains(&"DEPS_RELEVANT".to_string())
        );
        assert!(rejected_scope.extra.contains(&"INVALID_SCOPE".to_string()));

        let compatibility_graph = compatibility_client
            .get_graph_relationships(CompatibilityGetGraphRelationshipsRequest {
                node_id: ROOT_NODE_ID.to_string(),
                node_type: ROOT_LABEL.to_string(),
                depth: 9,
            })
            .await?
            .into_inner();
        assert_eq!(
            compatibility_graph
                .node
                .as_ref()
                .map(|node| node.id.as_str()),
            Some(ROOT_NODE_ID)
        );
        assert_eq!(
            compatibility_graph
                .node
                .as_ref()
                .map(|node| node.r#type.as_str()),
            Some(ROOT_LABEL)
        );
        assert!(
            compatibility_graph
                .neighbors
                .iter()
                .any(|node| node.id == TASK_ID)
        );
        assert!(
            compatibility_graph
                .neighbors
                .iter()
                .any(|node| node.id == DECISION_ID)
        );
        assert_eq!(compatibility_graph.neighbors.len(), EXPECTED_NEIGHBOR_COUNT);
        assert_eq!(
            compatibility_graph.relationships.len(),
            EXPECTED_RELATIONSHIP_COUNT
        );
        assert!(compatibility_graph.relationships.iter().any(|edge| {
            edge.from_node_id == DECISION_ID
                && edge.to_node_id == TASK_ID
                && edge.r#type == RELATION_IMPACTS
        }));
        assert!(compatibility_graph.relationships.iter().any(|edge| {
            edge.from_node_id == POWER_TASK_ID
                && edge.to_node_id == TASK_ID
                && edge.r#type == RELATION_DEPENDS_ON
        }));
        assert!(compatibility_graph.relationships.iter().any(|edge| {
            edge.from_node_id == JUMP_DECISION_ID
                && edge.to_node_id == DECISION_ID
                && edge.r#type == RELATION_DECISION_REQUIRES
        }));

        let compatibility_rehydrate = compatibility_client
            .rehydrate_session(CompatibilityRehydrateSessionRequest {
                case_id: ROOT_NODE_ID.to_string(),
                roles: vec![DEVELOPER_ROLE.to_string()],
                include_timeline: true,
                include_summaries: true,
                timeline_events: 7,
                persist_bundle: true,
                ttl_seconds: 300,
            })
            .await?
            .into_inner();
        assert_eq!(compatibility_rehydrate.case_id, ROOT_NODE_ID);
        let compatibility_pack = compatibility_rehydrate
            .packs
            .get(DEVELOPER_ROLE)
            .expect("developer pack should exist");
        assert_eq!(compatibility_pack.subtasks.len(), EXPECTED_TASK_COUNT);
        assert_eq!(compatibility_pack.decisions.len(), EXPECTED_DECISION_COUNT);
        assert_eq!(
            compatibility_pack.decision_deps.len(),
            EXPECTED_DECISION_EDGE_COUNT
        );
        assert_eq!(compatibility_pack.impacted.len(), EXPECTED_IMPACT_COUNT);
        assert_eq!(compatibility_pack.last_summary, ROOT_DETAIL);
        assert_eq!(
            compatibility_pack.token_budget_hint,
            EXPECTED_TOKEN_BUDGET_HINT
        );
        assert_eq!(
            compatibility_pack
                .plan_header
                .as_ref()
                .map(|header| header.total_subtasks),
            Some(EXPECTED_TASK_COUNT as i32)
        );
        assert_eq!(
            compatibility_pack
                .plan_header
                .as_ref()
                .map(|header| header.completed_subtasks),
            Some(EXPECTED_COMPLETED_TASK_COUNT)
        );
        assert!(compatibility_pack.subtasks.iter().any(|subtask| {
            subtask.subtask_id == TASK_ID && subtask.dependencies == vec![POWER_TASK_ID.to_string()]
        }));
        assert!(compatibility_pack.decisions.iter().any(|decision| {
            decision.id == DECISION_ID && decision.rationale == DECISION_DETAIL
        }));
        assert!(
            compatibility_pack.impacted.iter().any(|impact| {
                impact.decision_id == DECISION_ID && impact.subtask_id == TASK_ID
            })
        );
        assert!(compatibility_pack.decision_deps.iter().any(|edge| {
            edge.src_id == JUMP_DECISION_ID
                && edge.dst_id == DECISION_ID
                && edge.relation_type == RELATION_DECISION_REQUIRES
        }));
        let compatibility_stats = compatibility_rehydrate
            .stats
            .as_ref()
            .expect("compatibility stats should exist");
        assert_eq!(
            compatibility_stats.decisions,
            EXPECTED_DECISION_COUNT as i32
        );
        assert_eq!(
            compatibility_stats.decision_edges,
            EXPECTED_DECISION_EDGE_COUNT as i32
        );
        assert_eq!(compatibility_stats.impacts, EXPECTED_IMPACT_COUNT as i32);
        assert_eq!(compatibility_stats.events, 7);
        assert_eq!(compatibility_stats.roles, vec![DEVELOPER_ROLE.to_string()]);

        let compatibility_update = compatibility_client
            .update_context(CompatibilityUpdateContextRequest {
                story_id: ROOT_NODE_ID.to_string(),
                task_id: TASK_ID.to_string(),
                role: DEVELOPER_ROLE.to_string(),
                changes: vec![CompatibilityContextChange {
                    operation: "UPDATE".to_string(),
                    entity_type: "decision".to_string(),
                    entity_id: DECISION_ID.to_string(),
                    payload: "{\"status\":\"ACCEPTED\"}".to_string(),
                    reason: "documented".to_string(),
                }],
                timestamp: "2026-03-18T00:00:00Z".to_string(),
            })
            .await?
            .into_inner();
        assert_eq!(compatibility_update.version, 1);
        assert!(compatibility_update.hash.contains(ROOT_NODE_ID));

        let query_context = query_client
            .get_context(GetContextRequest {
                root_node_id: ROOT_NODE_ID.to_string(),
                role: DEVELOPER_ROLE.to_string(),
                phase: Phase::Build as i32,
                work_item_id: TASK_ID.to_string(),
                token_budget: 2048,
                requested_scopes: vec!["graph".to_string(), "decisions".to_string()],
                render_format: BundleRenderFormat::Structured as i32,
                include_debug_sections: true,
            })
            .await?
            .into_inner();
        let query_bundle = query_context.bundle.expect("query bundle should exist");
        assert_eq!(query_bundle.root_node_id, ROOT_NODE_ID);
        assert_eq!(query_bundle.bundles.len(), 1);
        let query_role_bundle = &query_bundle.bundles[0];
        assert_eq!(query_role_bundle.role, DEVELOPER_ROLE);
        assert_eq!(
            query_role_bundle.neighbor_nodes.len(),
            EXPECTED_NEIGHBOR_COUNT
        );
        assert_eq!(
            query_role_bundle.relationships.len(),
            EXPECTED_RELATIONSHIP_COUNT
        );
        assert_eq!(query_role_bundle.node_details.len(), EXPECTED_DETAIL_COUNT);
        assert!(
            query_role_bundle
                .neighbor_nodes
                .iter()
                .any(|node| node.node_id == TASK_ID)
        );
        assert!(
            query_role_bundle
                .node_details
                .iter()
                .any(|detail| detail.node_id == TASK_ID)
        );
        assert!(query_role_bundle.relationships.iter().any(|edge| {
            edge.source_node_id == DECISION_ID
                && edge.target_node_id == TASK_ID
                && edge.relationship_type == RELATION_IMPACTS
        }));
        assert!(query_role_bundle.relationships.iter().any(|edge| {
            edge.source_node_id == POWER_TASK_ID
                && edge.target_node_id == TASK_ID
                && edge.relationship_type == RELATION_DEPENDS_ON
        }));
        assert!(query_role_bundle.relationships.iter().any(|edge| {
            edge.source_node_id == JUMP_DECISION_ID
                && edge.target_node_id == DECISION_ID
                && edge.relationship_type == RELATION_DECISION_REQUIRES
        }));
        let rendered_query_context = query_context
            .rendered
            .expect("rendered context should exist");
        assert!(rendered_query_context.content.contains(ROOT_TITLE));
        assert!(rendered_query_context.content.contains(ROOT_DETAIL));
        assert!(rendered_query_context.content.contains(TASK_DETAIL));
        assert!(rendered_query_context.content.contains(DECISION_DETAIL));
        assert!(
            rendered_query_context
                .content
                .contains(PROPULSION_SUBSYSTEM_TITLE)
        );
        assert!(
            rendered_query_context
                .content
                .contains(CHIEF_ENGINEER_TITLE)
        );

        let query_rehydrate = query_client
            .rehydrate_session(rehydration_proto::v1alpha1::RehydrateSessionRequest {
                root_node_id: ROOT_NODE_ID.to_string(),
                roles: vec![DEVELOPER_ROLE.to_string()],
                include_timeline: true,
                include_summaries: true,
                timeline_window: 11,
                persist_snapshot: true,
                snapshot_ttl: Some(Duration {
                    seconds: 600,
                    nanos: 0,
                }),
            })
            .await?
            .into_inner();
        assert!(query_rehydrate.snapshot_persisted);
        assert_eq!(
            query_rehydrate.snapshot_id,
            format!("snapshot:{ROOT_NODE_ID}:{DEVELOPER_ROLE}")
        );
        let query_rehydrate_bundle = query_rehydrate
            .bundle
            .expect("rehydrated bundle should exist");
        assert_eq!(query_rehydrate_bundle.root_node_id, ROOT_NODE_ID);
        let query_rehydrate_stats = query_rehydrate_bundle
            .stats
            .as_ref()
            .expect("rehydrated stats should exist");
        assert_eq!(query_rehydrate_stats.nodes, EXPECTED_SELECTED_NODE_COUNT);
        assert_eq!(
            query_rehydrate_stats.relationships,
            EXPECTED_SELECTED_RELATIONSHIP_COUNT
        );
        assert_eq!(
            query_rehydrate_stats.detailed_nodes,
            EXPECTED_DETAIL_COUNT as u32
        );
        assert_eq!(query_rehydrate_stats.timeline_events, 11);
        assert_eq!(query_rehydrate_bundle.bundles.len(), 1);
        assert_eq!(
            query_rehydrate_bundle.bundles[0].neighbor_nodes.len(),
            EXPECTED_NEIGHBOR_COUNT
        );
        assert_eq!(
            query_rehydrate_bundle.bundles[0].relationships.len(),
            EXPECTED_RELATIONSHIP_COUNT
        );
        assert_eq!(
            query_rehydrate_bundle.bundles[0].node_details.len(),
            EXPECTED_DETAIL_COUNT
        );

        let command_update = command_client
            .update_context(UpdateContextRequest {
                root_node_id: ROOT_NODE_ID.to_string(),
                role: DEVELOPER_ROLE.to_string(),
                work_item_id: TASK_ID.to_string(),
                changes: vec![ContextChange {
                    operation: ContextChangeOperation::Update as i32,
                    entity_kind: "node_detail".to_string(),
                    entity_id: TASK_ID.to_string(),
                    payload_json: "{\"status\":\"READY\"}".to_string(),
                    reason: "full-journey-tls".to_string(),
                    scopes: vec!["graph".to_string(), "details".to_string()],
                }],
                metadata: Some(CommandMetadata {
                    idempotency_key: "idem-kernel-full-journey-tls".to_string(),
                    correlation_id: "corr-kernel-full-journey-tls".to_string(),
                    causation_id: "cause-kernel-full-journey-tls".to_string(),
                    requested_by: "kernel-e2e-tls".to_string(),
                    requested_at: None,
                }),
                precondition: Some(RevisionPrecondition {
                    expected_revision: 41,
                    expected_content_hash: "expected-hash".to_string(),
                }),
                persist_snapshot: true,
            })
            .await?
            .into_inner();
        assert_eq!(
            command_update
                .accepted_version
                .as_ref()
                .map(|version| version.revision),
            Some(42)
        );
        assert!(command_update.warnings.is_empty());
        assert!(command_update.snapshot_persisted);

        let projection_status = admin_client
            .get_projection_status(GetProjectionStatusRequest {
                consumer_names: vec!["context-projection".to_string()],
            })
            .await?
            .into_inner();
        assert_eq!(projection_status.projections.len(), 1);
        assert_eq!(
            projection_status.projections[0].consumer_name,
            "context-projection"
        );
        assert!(projection_status.projections[0].healthy);

        let bundle_snapshot = admin_client
            .get_bundle_snapshot(GetBundleSnapshotRequest {
                root_node_id: ROOT_NODE_ID.to_string(),
                role: DEVELOPER_ROLE.to_string(),
            })
            .await?
            .into_inner()
            .snapshot
            .expect("admin snapshot should exist");
        assert_eq!(
            bundle_snapshot.snapshot_id,
            format!("snapshot:{ROOT_NODE_ID}:{DEVELOPER_ROLE}")
        );
        assert_eq!(
            bundle_snapshot.ttl.as_ref().map(|ttl| ttl.seconds),
            Some(900)
        );
        assert_eq!(
            bundle_snapshot
                .bundle
                .as_ref()
                .map(|bundle| bundle.root_node_id.as_str()),
            Some(ROOT_NODE_ID)
        );
        let snapshot_bundle = bundle_snapshot
            .bundle
            .as_ref()
            .expect("snapshot bundle should exist");
        let snapshot_stats = snapshot_bundle
            .stats
            .as_ref()
            .expect("snapshot stats should exist");
        assert_eq!(snapshot_stats.nodes, EXPECTED_SELECTED_NODE_COUNT);
        assert_eq!(
            snapshot_stats.relationships,
            EXPECTED_SELECTED_RELATIONSHIP_COUNT
        );
        assert_eq!(snapshot_stats.detailed_nodes, EXPECTED_DETAIL_COUNT as u32);

        let admin_graph = admin_client
            .get_graph_relationships(GetGraphRelationshipsRequest {
                node_id: ROOT_NODE_ID.to_string(),
                node_kind: ROOT_LABEL.to_string(),
                depth: 3,
                include_reverse_edges: false,
            })
            .await?
            .into_inner();
        assert_eq!(
            admin_graph.root.as_ref().map(|node| node.node_id.as_str()),
            Some(ROOT_NODE_ID)
        );
        assert!(
            admin_graph
                .neighbors
                .iter()
                .any(|node| node.node_id == TASK_ID)
        );
        assert_eq!(admin_graph.neighbors.len(), EXPECTED_NEIGHBOR_COUNT);
        assert_eq!(admin_graph.relationships.len(), EXPECTED_RELATIONSHIP_COUNT);
        assert!(admin_graph.relationships.iter().any(|edge| {
            edge.source_node_id == DECISION_ID
                && edge.target_node_id == TASK_ID
                && edge.relationship_type == RELATION_IMPACTS
        }));
        assert!(admin_graph.relationships.iter().any(|edge| {
            edge.source_node_id == POWER_TASK_ID
                && edge.target_node_id == TASK_ID
                && edge.relationship_type == RELATION_DEPENDS_ON
        }));
        assert!(admin_graph.relationships.iter().any(|edge| {
            edge.source_node_id == JUMP_DECISION_ID
                && edge.target_node_id == DECISION_ID
                && edge.relationship_type == RELATION_DECISION_REQUIRES
        }));

        let diagnostics = admin_client
            .get_rehydration_diagnostics(GetRehydrationDiagnosticsRequest {
                root_node_id: ROOT_NODE_ID.to_string(),
                roles: vec![DEVELOPER_ROLE.to_string(), "reviewer".to_string()],
                phase: Phase::Build as i32,
            })
            .await?
            .into_inner();
        assert_eq!(diagnostics.diagnostics.len(), 2);
        assert!(
            diagnostics
                .diagnostics
                .iter()
                .all(|item| item.selected_nodes == EXPECTED_SELECTED_NODE_COUNT
                    && item.selected_relationships == EXPECTED_SELECTED_RELATIONSHIP_COUNT
                    && item.detailed_nodes == EXPECTED_DETAIL_COUNT as u32
                    && item.estimated_tokens > EXPECTED_SELECTED_NODE_COUNT * 10)
        );
        assert!(
            diagnostics
                .diagnostics
                .iter()
                .all(|item| item.notes.iter().any(|note| note.starts_with("phase=")))
        );

        let replay = admin_client
            .replay_projection(ReplayProjectionRequest {
                consumer_name: "context-projection".to_string(),
                stream_name: "rehydration.graph.node.materialized".to_string(),
                starting_after: String::new(),
                max_events: 25,
                replay_mode: ReplayMode::DryRun as i32,
                requested_by: "kernel-e2e-tls".to_string(),
            })
            .await?
            .into_inner();
        assert_eq!(replay.consumer_name, "context-projection");
        assert_eq!(replay.accepted_events, 25);
        assert_eq!(replay.replay_mode, ReplayMode::DryRun as i32);

        Ok::<(), Box<dyn Error + Send + Sync>>(())
    }
    .await;

    fixture.shutdown().await?;
    result
}

async fn wait_for_full_compatibility_graph(
    mut compatibility_client: rehydration_proto::fleet_context_v1::context_service_client::ContextServiceClient<
        Channel,
    >,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut last_error: Option<Box<dyn Error + Send + Sync>> = None;

    for _ in 0..40 {
        match compatibility_client
            .get_graph_relationships(CompatibilityGetGraphRelationshipsRequest {
                node_id: ROOT_NODE_ID.to_string(),
                node_type: ROOT_LABEL.to_string(),
                depth: 9,
            })
            .await
        {
            Ok(response) => {
                let graph = response.into_inner();
                if graph.neighbors.len() == EXPECTED_NEIGHBOR_COUNT
                    && graph.relationships.len() == EXPECTED_RELATIONSHIP_COUNT
                {
                    return Ok(());
                }
            }
            Err(error) => last_error = Some(Box::new(error)),
        }

        sleep(StdDuration::from_millis(200)).await;
    }

    Err(last_error.unwrap_or_else(|| {
        "compatibility graph did not reach expected size before timeout"
            .to_string()
            .into()
    }))
}
