mod support;

use std::collections::BTreeMap;
use std::error::Error;

use rehydration_adapter_neo4j::Neo4jProjectionReader;
use rehydration_adapter_valkey::{ValkeyNodeDetailStore, ValkeySnapshotStore};
use rehydration_ports::{
    NodeDetailProjection, NodeProjection, NodeRelationProjection, ProjectionMutation,
    ProjectionWriter,
};
use rehydration_proto::fleet_context_v1::{
    GetContextRequest, GetGraphRelationshipsRequest, RehydrateSessionRequest,
    context_service_client::ContextServiceClient,
};

use self::support::containers::{
    NEO4J_INTERNAL_PORT, NEO4J_PASSWORD, VALKEY_INTERNAL_PORT, clear_neo4j, start_neo4j_container,
    start_valkey_container,
};
use self::support::grpc_runtime::{RunningGrpcServer, stop_server};
use self::support::resp::{get_json_value, get_ttl};

#[tokio::test]
async fn compatibility_get_context_and_graph_relationships_use_real_projection_data()
-> Result<(), Box<dyn Error + Send + Sync>> {
    let neo4j = start_neo4j_container().await?;
    let valkey = start_valkey_container().await?;

    let neo4j_host = neo4j.get_host().await?;
    let neo4j_port = neo4j.get_host_port_ipv4(NEO4J_INTERNAL_PORT).await?;
    let valkey_host = valkey.get_host().await?;
    let valkey_port = valkey.get_host_port_ipv4(VALKEY_INTERNAL_PORT).await?;

    let neo4j_seed_uri = format!("neo4j://{neo4j_host}:{neo4j_port}");
    clear_neo4j(neo4j_seed_uri.clone()).await?;

    let graph_store = Neo4jProjectionReader::new(format!(
        "neo4j://neo4j:{NEO4J_PASSWORD}@{neo4j_host}:{neo4j_port}"
    ))?;
    let detail_store = ValkeyNodeDetailStore::new(format!(
        "redis://{valkey_host}:{valkey_port}?key_prefix=rehydration:detail&ttl_seconds=120"
    ))?;
    let snapshot_store = ValkeySnapshotStore::new(format!(
        "redis://{valkey_host}:{valkey_port}?key_prefix=rehydration:snapshot&ttl_seconds=120"
    ))?;

    seed_projection_graph(&graph_store).await?;
    seed_node_details(&detail_store).await?;

    let server = RunningGrpcServer::start(
        graph_store.clone(),
        detail_store.clone(),
        snapshot_store.clone(),
    )
    .await?;
    let channel = server.connect_channel().await?;
    let mut client = ContextServiceClient::new(channel);

    let get_context = client
        .get_context(GetContextRequest {
            story_id: "story-123".to_string(),
            role: "developer".to_string(),
            phase: "BUILD".to_string(),
            subtask_id: String::new(),
            token_budget: 2048,
        })
        .await?
        .into_inner();

    assert!(get_context.context.contains("Hydrate projection shell"));
    assert!(get_context.context.contains("Extended story detail"));
    assert_eq!(get_context.blocks.expect("blocks").system, "role=developer");

    let graph_relationships = client
        .get_graph_relationships(GetGraphRelationshipsRequest {
            node_id: "story-123".to_string(),
            node_type: "Story".to_string(),
            depth: 9,
        })
        .await?
        .into_inner();

    assert!(graph_relationships.success);
    assert_eq!(graph_relationships.node.expect("root node").id, "story-123");
    assert_eq!(graph_relationships.neighbors.len(), 2);
    assert_eq!(graph_relationships.relationships.len(), 2);

    stop_server(server).await?;
    Ok(())
}

#[tokio::test]
async fn compatibility_rehydrate_session_persists_snapshot_in_valkey()
-> Result<(), Box<dyn Error + Send + Sync>> {
    let neo4j = start_neo4j_container().await?;
    let valkey = start_valkey_container().await?;

    let neo4j_host = neo4j.get_host().await?;
    let neo4j_port = neo4j.get_host_port_ipv4(NEO4J_INTERNAL_PORT).await?;
    let valkey_host = valkey.get_host().await?;
    let valkey_port = valkey.get_host_port_ipv4(VALKEY_INTERNAL_PORT).await?;
    let valkey_address = format!("{valkey_host}:{valkey_port}");

    clear_neo4j(format!("neo4j://{neo4j_host}:{neo4j_port}")).await?;

    let graph_store = Neo4jProjectionReader::new(format!(
        "neo4j://neo4j:{NEO4J_PASSWORD}@{neo4j_host}:{neo4j_port}"
    ))?;
    let detail_store = ValkeyNodeDetailStore::new(format!(
        "redis://{valkey_host}:{valkey_port}?key_prefix=rehydration:detail&ttl_seconds=120"
    ))?;
    let snapshot_store = ValkeySnapshotStore::new(format!(
        "redis://{valkey_host}:{valkey_port}?key_prefix=rehydration:snapshot&ttl_seconds=120"
    ))?;

    seed_projection_graph(&graph_store).await?;
    seed_node_details(&detail_store).await?;

    let server = RunningGrpcServer::start(
        graph_store.clone(),
        detail_store.clone(),
        snapshot_store.clone(),
    )
    .await?;
    let channel = server.connect_channel().await?;
    let mut client = ContextServiceClient::new(channel);

    let response = client
        .rehydrate_session(RehydrateSessionRequest {
            case_id: "story-123".to_string(),
            roles: vec!["developer".to_string()],
            include_timeline: true,
            include_summaries: true,
            timeline_events: 7,
            persist_bundle: true,
            ttl_seconds: 120,
        })
        .await?
        .into_inner();

    assert_eq!(response.case_id, "story-123");
    assert!(response.generated_at_ms > 0);
    assert!(response.packs.contains_key("developer"));
    assert_eq!(response.stats.expect("stats").events, 7);

    let snapshot_key = "rehydration:snapshot:story-123:developer";
    let snapshot = get_json_value(&valkey_address, snapshot_key).await?;
    let ttl = get_ttl(&valkey_address, snapshot_key).await?;

    assert_eq!(snapshot["root_node_id"], "story-123");
    assert_eq!(snapshot["role"], "developer");
    assert!((1..=120).contains(&ttl));

    stop_server(server).await?;
    Ok(())
}

async fn seed_projection_graph(
    graph_store: &Neo4jProjectionReader,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    graph_store
        .apply_mutations(vec![
            ProjectionMutation::UpsertNode(NodeProjection {
                node_id: "story-123".to_string(),
                node_kind: "story".to_string(),
                title: "Hydrate projection shell".to_string(),
                summary: "Root story summary".to_string(),
                status: "ACTIVE".to_string(),
                labels: vec!["Story".to_string()],
                properties: BTreeMap::from([
                    ("created_by".to_string(), "planner".to_string()),
                    ("plan_id".to_string(), "plan-42".to_string()),
                ]),
            }),
            ProjectionMutation::UpsertNode(NodeProjection {
                node_id: "decision-1".to_string(),
                node_kind: "decision".to_string(),
                title: "Use compatibility shell".to_string(),
                summary: "Decision summary".to_string(),
                status: "ACCEPTED".to_string(),
                labels: vec!["Decision".to_string()],
                properties: BTreeMap::new(),
            }),
            ProjectionMutation::UpsertNode(NodeProjection {
                node_id: "task-1".to_string(),
                node_kind: "task".to_string(),
                title: "Wire gRPC facade".to_string(),
                summary: "Task summary".to_string(),
                status: "READY".to_string(),
                labels: vec!["Task".to_string()],
                properties: BTreeMap::from([
                    ("role".to_string(), "DEV".to_string()),
                    ("priority".to_string(), "3".to_string()),
                ]),
            }),
            ProjectionMutation::UpsertNodeRelation(NodeRelationProjection {
                source_node_id: "story-123".to_string(),
                target_node_id: "decision-1".to_string(),
                relation_type: "records".to_string(),
            }),
            ProjectionMutation::UpsertNodeRelation(NodeRelationProjection {
                source_node_id: "story-123".to_string(),
                target_node_id: "task-1".to_string(),
                relation_type: "has_task".to_string(),
            }),
            ProjectionMutation::UpsertNodeRelation(NodeRelationProjection {
                source_node_id: "decision-1".to_string(),
                target_node_id: "task-1".to_string(),
                relation_type: "IMPACTS".to_string(),
            }),
        ])
        .await?;

    Ok(())
}

async fn seed_node_details(
    detail_store: &ValkeyNodeDetailStore,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    detail_store
        .apply_mutations(vec![
            ProjectionMutation::UpsertNodeDetail(NodeDetailProjection {
                node_id: "story-123".to_string(),
                detail: "Extended story detail".to_string(),
                content_hash: "hash-story".to_string(),
                revision: 2,
            }),
            ProjectionMutation::UpsertNodeDetail(NodeDetailProjection {
                node_id: "decision-1".to_string(),
                detail: "Detailed rationale".to_string(),
                content_hash: "hash-decision".to_string(),
                revision: 3,
            }),
        ])
        .await?;

    Ok(())
}
