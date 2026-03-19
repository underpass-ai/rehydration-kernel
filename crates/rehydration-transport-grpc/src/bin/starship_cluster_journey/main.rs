use std::env;
use std::error::Error;
use std::time::Duration as StdDuration;
use std::time::{SystemTime, UNIX_EPOCH};

use rehydration_proto::fleet_context_v1::{
    GetContextRequest as CompatibilityGetContextRequest,
    context_service_client::ContextServiceClient,
};
use rehydration_proto::v1alpha1::{
    BundleRenderFormat, GetBundleSnapshotRequest, GetContextRequest, GetGraphRelationshipsRequest,
    GetProjectionStatusRequest, GetRehydrationDiagnosticsRequest, Phase, RehydrateSessionRequest,
    context_admin_service_client::ContextAdminServiceClient,
    context_query_service_client::ContextQueryServiceClient,
};
use rehydration_transport_grpc::starship_e2e::{
    CHIEF_ENGINEER_TITLE, DECISION_DETAIL, DECISION_ID, DECISION_TITLE, DEFAULT_SUBJECT_PREFIX,
    EXPECTED_DETAIL_COUNT, EXPECTED_NEIGHBOR_COUNT, EXPECTED_RELATIONSHIP_COUNT,
    EXPECTED_SELECTED_NODE_COUNT, EXPECTED_SELECTED_RELATIONSHIP_COUNT, POWER_TASK_ID,
    PROPULSION_SUBSYSTEM_TITLE, RELATION_DECISION_REQUIRES, RELATION_DEPENDS_ON, RELATION_IMPACTS,
    ROOT_DETAIL, ROOT_LABEL, ROOT_NODE_ID, ROOT_TITLE, TASK_DETAIL, TASK_ID, TASK_TITLE,
    publish_projection_events_for_run,
};
use serde::Serialize;
use tokio::time::sleep;
use tonic::transport::Channel;

const DEFAULT_GRPC_ENDPOINT: &str = "http://127.0.0.1:50054";
const DEFAULT_NATS_URL: &str = "nats://127.0.0.1:4222";
const DEFAULT_ROLE: &str = "developer";
const REVIEWER_ROLE: &str = "reviewer";
const DEFAULT_PHASE: i32 = Phase::Build as i32;
const DEFAULT_TOKEN_BUDGET: u32 = 2048;
const DEFAULT_TIMELINE_WINDOW: u32 = 11;
const DEFAULT_SNAPSHOT_TTL_SECONDS: i64 = 900;

#[derive(Serialize)]
struct DiagnosticSummary {
    role: String,
    selected_nodes: u32,
    selected_relationships: u32,
    detailed_nodes: u32,
    estimated_tokens: u32,
}

#[derive(Serialize)]
struct VerificationSummary {
    release_root: String,
    neighbors: usize,
    relationships: usize,
    details: usize,
    rendered_token_count: u32,
    compatibility_token_count: i32,
    projection_healthy: bool,
    snapshot_id: String,
    diagnostics: Vec<DiagnosticSummary>,
}

struct AppConfig {
    grpc_endpoint: String,
    nats_url: String,
    subject_prefix: String,
}

impl AppConfig {
    fn from_env() -> Self {
        Self {
            grpc_endpoint: env::var("CLUSTER_STARSHIP_GRPC_ENDPOINT")
                .unwrap_or_else(|_| DEFAULT_GRPC_ENDPOINT.to_string()),
            nats_url: env::var("CLUSTER_STARSHIP_NATS_URL")
                .unwrap_or_else(|_| DEFAULT_NATS_URL.to_string()),
            subject_prefix: env::var("CLUSTER_STARSHIP_SUBJECT_PREFIX")
                .unwrap_or_else(|_| DEFAULT_SUBJECT_PREFIX.to_string()),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let config = AppConfig::from_env();
    let run_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().to_string())
        .unwrap_or_else(|_| "cluster-run".to_string());

    let publisher = async_nats::connect(config.nats_url.clone()).await?;
    publish_projection_events_for_run(&publisher, &config.subject_prefix, &run_id).await?;

    let compatibility_client = ContextServiceClient::connect(config.grpc_endpoint.clone()).await?;
    let query_client = ContextQueryServiceClient::connect(config.grpc_endpoint.clone()).await?;
    let admin_client = ContextAdminServiceClient::connect(config.grpc_endpoint).await?;

    wait_for_context_ready(query_client.clone()).await?;
    let summary = verify(compatibility_client, query_client, admin_client).await?;
    println!("{}", serde_json::to_string_pretty(&summary)?);

    Ok(())
}

async fn wait_for_context_ready(
    mut query_client: ContextQueryServiceClient<Channel>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut last_error: Option<Box<dyn Error + Send + Sync>> = None;

    for _ in 0..60 {
        match query_client
            .get_context(GetContextRequest {
                root_node_id: ROOT_NODE_ID.to_string(),
                role: DEFAULT_ROLE.to_string(),
                phase: DEFAULT_PHASE,
                work_item_id: TASK_ID.to_string(),
                token_budget: DEFAULT_TOKEN_BUDGET,
                requested_scopes: vec!["graph".to_string(), "decisions".to_string()],
                render_format: BundleRenderFormat::Structured as i32,
                include_debug_sections: true,
                depth: 3,
            })
            .await
        {
            Ok(response) => {
                let response = response.into_inner();
                if let Some(bundle) = response.bundle {
                    let role_bundle = bundle.bundles.first();
                    if bundle.root_node_id == ROOT_NODE_ID
                        && role_bundle.is_some_and(|item| {
                            item.neighbor_nodes.len() == EXPECTED_NEIGHBOR_COUNT
                                && item.relationships.len() == EXPECTED_RELATIONSHIP_COUNT
                                && item.node_details.len() == EXPECTED_DETAIL_COUNT
                        })
                    {
                        return Ok(());
                    }
                }
            }
            Err(error) => {
                last_error = Some(Box::new(error));
            }
        }

        sleep(StdDuration::from_millis(500)).await;
    }

    Err(last_error.unwrap_or_else(|| {
        "starship context projection did not become ready before timeout"
            .to_string()
            .into()
    }))
}

async fn verify(
    mut compatibility_client: ContextServiceClient<Channel>,
    mut query_client: ContextQueryServiceClient<Channel>,
    mut admin_client: ContextAdminServiceClient<Channel>,
) -> Result<VerificationSummary, Box<dyn Error + Send + Sync>> {
    let compatibility_context = compatibility_client
        .get_context(CompatibilityGetContextRequest {
            story_id: ROOT_NODE_ID.to_string(),
            role: DEFAULT_ROLE.to_string(),
            phase: "BUILD".to_string(),
            subtask_id: TASK_ID.to_string(),
            token_budget: DEFAULT_TOKEN_BUDGET as i32,
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

    let query_context = query_client
        .get_context(GetContextRequest {
            root_node_id: ROOT_NODE_ID.to_string(),
            role: DEFAULT_ROLE.to_string(),
            phase: DEFAULT_PHASE,
            work_item_id: TASK_ID.to_string(),
            token_budget: DEFAULT_TOKEN_BUDGET,
            requested_scopes: vec!["graph".to_string(), "decisions".to_string()],
            render_format: BundleRenderFormat::Structured as i32,
            include_debug_sections: true,
            depth: 3,
        })
        .await?
        .into_inner();
    let rendered_query_context = query_context
        .rendered
        .expect("rendered context should exist");
    assert!(rendered_query_context.content.contains(ROOT_TITLE));
    assert!(rendered_query_context.content.contains(ROOT_DETAIL));
    assert!(rendered_query_context.content.contains(TASK_DETAIL));
    assert!(rendered_query_context.content.contains(DECISION_DETAIL));
    let query_bundle = query_context.bundle.expect("query bundle should exist");
    assert_eq!(query_bundle.root_node_id, ROOT_NODE_ID);
    assert_eq!(query_bundle.bundles.len(), 1);
    let query_role_bundle = &query_bundle.bundles[0];
    assert_eq!(
        query_role_bundle.neighbor_nodes.len(),
        EXPECTED_NEIGHBOR_COUNT
    );
    assert_eq!(
        query_role_bundle.relationships.len(),
        EXPECTED_RELATIONSHIP_COUNT
    );
    assert_eq!(query_role_bundle.node_details.len(), EXPECTED_DETAIL_COUNT);
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
        edge.source_node_id == "decision:delay-jump-window"
            && edge.target_node_id == DECISION_ID
            && edge.relationship_type == RELATION_DECISION_REQUIRES
    }));

    let query_rehydrate = query_client
        .rehydrate_session(RehydrateSessionRequest {
            root_node_id: ROOT_NODE_ID.to_string(),
            roles: vec![DEFAULT_ROLE.to_string()],
            include_timeline: true,
            include_summaries: true,
            timeline_window: DEFAULT_TIMELINE_WINDOW,
            persist_snapshot: true,
            snapshot_ttl: Some(prost_types::Duration {
                seconds: DEFAULT_SNAPSHOT_TTL_SECONDS,
                nanos: 0,
            }),
        })
        .await?
        .into_inner();
    assert!(query_rehydrate.snapshot_persisted);
    assert_eq!(
        query_rehydrate.snapshot_id,
        format!("snapshot:{ROOT_NODE_ID}:{DEFAULT_ROLE}")
    );
    let query_rehydrate_bundle = query_rehydrate
        .bundle
        .expect("rehydrated bundle should exist");
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
            role: DEFAULT_ROLE.to_string(),
        })
        .await?
        .into_inner()
        .snapshot
        .expect("admin snapshot should exist");
    assert_eq!(
        bundle_snapshot.snapshot_id,
        format!("snapshot:{ROOT_NODE_ID}:{DEFAULT_ROLE}")
    );
    assert_eq!(
        bundle_snapshot.ttl.as_ref().map(|ttl| ttl.seconds),
        Some(DEFAULT_SNAPSHOT_TTL_SECONDS)
    );

    let admin_graph = admin_client
        .get_graph_relationships(GetGraphRelationshipsRequest {
            node_id: ROOT_NODE_ID.to_string(),
            node_kind: ROOT_LABEL.to_string(),
            depth: 3,
            include_reverse_edges: false,
        })
        .await?
        .into_inner();
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
        edge.source_node_id == "decision:delay-jump-window"
            && edge.target_node_id == DECISION_ID
            && edge.relationship_type == RELATION_DECISION_REQUIRES
    }));

    let diagnostics = admin_client
        .get_rehydration_diagnostics(GetRehydrationDiagnosticsRequest {
            root_node_id: ROOT_NODE_ID.to_string(),
            roles: vec![DEFAULT_ROLE.to_string(), REVIEWER_ROLE.to_string()],
            phase: DEFAULT_PHASE,
        })
        .await?
        .into_inner();
    assert_eq!(diagnostics.diagnostics.len(), 2);
    assert!(diagnostics.diagnostics.iter().all(|item| {
        item.selected_nodes == EXPECTED_SELECTED_NODE_COUNT
            && item.selected_relationships == EXPECTED_SELECTED_RELATIONSHIP_COUNT
            && item.detailed_nodes == EXPECTED_DETAIL_COUNT as u32
            && item.estimated_tokens > EXPECTED_SELECTED_NODE_COUNT * 10
    }));

    Ok(VerificationSummary {
        release_root: ROOT_NODE_ID.to_string(),
        neighbors: query_role_bundle.neighbor_nodes.len(),
        relationships: query_role_bundle.relationships.len(),
        details: query_role_bundle.node_details.len(),
        rendered_token_count: rendered_query_context.token_count,
        compatibility_token_count: compatibility_context.token_count,
        projection_healthy: projection_status.projections[0].healthy,
        snapshot_id: bundle_snapshot.snapshot_id,
        diagnostics: diagnostics
            .diagnostics
            .into_iter()
            .map(|item| DiagnosticSummary {
                role: item.role,
                selected_nodes: item.selected_nodes,
                selected_relationships: item.selected_relationships,
                detailed_nodes: item.detailed_nodes,
                estimated_tokens: item.estimated_tokens,
            })
            .collect(),
    })
}
