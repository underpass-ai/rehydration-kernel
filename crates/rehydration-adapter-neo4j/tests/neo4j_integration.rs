use std::error::Error;
use std::time::Duration;

use neo4rs::{Graph, query};
use rehydration_adapter_neo4j::Neo4jProjectionReader;
use rehydration_domain::{CaseId, Role};
use rehydration_ports::{
    NodeProjection, NodeRelationProjection, ProjectionMutation, ProjectionReader, ProjectionWriter,
};
use testcontainers::{GenericImage, ImageExt, core::IntoContainerPort, runners::AsyncRunner};
use tokio::time::sleep;

const NEO4J_INTERNAL_PORT: u16 = 7687;
const NEO4J_IMAGE: &str = "docker.io/neo4j";
const NEO4J_TAG: &str = "5.26.0-community";
const NEO4J_PASSWORD: &str = "underpass-test-password";

#[tokio::test]
async fn load_pack_reads_role_context_projection() -> Result<(), Box<dyn Error + Send + Sync>> {
    let container = GenericImage::new(NEO4J_IMAGE, NEO4J_TAG)
        .with_exposed_port(NEO4J_INTERNAL_PORT.tcp())
        .with_env_var("NEO4J_AUTH", format!("neo4j/{NEO4J_PASSWORD}"))
        .start()
        .await?;

    let host = container.get_host().await?;
    let port = container.get_host_port_ipv4(NEO4J_INTERNAL_PORT).await?;
    let graph =
        connect_with_retry(format!("neo4j://{host}:{port}"), "neo4j", NEO4J_PASSWORD).await?;

    seed_projection(&graph).await?;

    let reader =
        Neo4jProjectionReader::new(format!("neo4j://neo4j:{NEO4J_PASSWORD}@{host}:{port}"))?;
    let pack = reader
        .load_pack(&CaseId::new("case-123")?, &Role::new("developer")?)
        .await?
        .expect("seeded projection should load");

    assert_eq!(pack.role().as_str(), "developer");
    assert_eq!(pack.case_header().title(), "Graph-backed case");
    assert_eq!(pack.case_header().created_by(), "planner");
    assert_eq!(
        pack.latest_summary(),
        "Prioritize the read path before replay"
    );
    assert_eq!(pack.token_budget_hint(), 6144);

    let plan_header = pack.plan_header().expect("plan header should be present");
    assert_eq!(plan_header.plan_id(), "plan-123");
    assert_eq!(plan_header.revision(), 4);
    assert_eq!(plan_header.work_items_total(), 2);
    assert_eq!(plan_header.work_items_completed(), 1);

    assert_eq!(pack.work_items().len(), 2);
    assert_eq!(pack.work_items()[0].work_item_id(), "story-002");
    assert_eq!(
        pack.work_items()[0].dependency_ids(),
        &["story-001".to_string()]
    );
    assert_eq!(pack.work_items()[1].work_item_id(), "story-001");

    assert_eq!(pack.decisions().len(), 2);
    assert_eq!(pack.decisions()[0].decision_id(), "decision-002");
    assert_eq!(pack.decision_relations().len(), 1);
    assert_eq!(pack.decision_relations()[0].relation_type(), "depends_on");
    assert_eq!(pack.impacts().len(), 1);
    assert_eq!(pack.impacts()[0].work_item_id(), "story-002");
    assert_eq!(pack.milestones().len(), 1);
    assert_eq!(pack.milestones()[0].milestone_type(), "phase_transition");

    Ok(())
}

#[tokio::test]
async fn apply_mutations_persists_generic_nodes_and_relations()
-> Result<(), Box<dyn Error + Send + Sync>> {
    let container = GenericImage::new(NEO4J_IMAGE, NEO4J_TAG)
        .with_exposed_port(NEO4J_INTERNAL_PORT.tcp())
        .with_env_var("NEO4J_AUTH", format!("neo4j/{NEO4J_PASSWORD}"))
        .start()
        .await?;

    let host = container.get_host().await?;
    let port = container.get_host_port_ipv4(NEO4J_INTERNAL_PORT).await?;
    let graph =
        connect_with_retry(format!("neo4j://{host}:{port}"), "neo4j", NEO4J_PASSWORD).await?;
    graph.run(query("MATCH (n) DETACH DELETE n")).await?;

    let store =
        Neo4jProjectionReader::new(format!("neo4j://neo4j:{NEO4J_PASSWORD}@{host}:{port}"))?;
    store
        .apply_mutations(vec![
            ProjectionMutation::UpsertNode(NodeProjection {
                node_id: "node-123".to_string(),
                node_kind: "capability".to_string(),
                title: "Projection consumer foundation".to_string(),
                summary: "Node centric projection input".to_string(),
                status: "ACTIVE".to_string(),
                labels: vec!["projection".to_string(), "foundation".to_string()],
                properties: std::collections::BTreeMap::from([(
                    "phase".to_string(),
                    "build".to_string(),
                )]),
            }),
            ProjectionMutation::UpsertNodeRelation(NodeRelationProjection {
                source_node_id: "node-123".to_string(),
                target_node_id: "node-122".to_string(),
                relation_type: "depends_on".to_string(),
            }),
        ])
        .await?;

    let node_row = single_row(
        &graph,
        query(
            "
MATCH (node:ProjectionNode {node_id: $node_id})
RETURN node.node_kind AS node_kind,
       node.title AS title,
       node.status AS status,
       node.node_labels AS node_labels,
       node.properties_json AS properties_json
            ",
        )
        .param("node_id", "node-123"),
    )
    .await?;

    let relation_row = single_row(
        &graph,
        query(
            "
MATCH (:ProjectionNode {node_id: $source_node_id})-[edge:RELATED_TO {relation_type: $relation_type}]->(:ProjectionNode {node_id: $target_node_id})
RETURN count(edge) AS edge_count
            ",
        )
        .param("source_node_id", "node-123")
        .param("target_node_id", "node-122")
        .param("relation_type", "depends_on"),
    )
    .await?;

    let node_kind: String = node_row.get("node_kind")?;
    let title: String = node_row.get("title")?;
    let status: String = node_row.get("status")?;
    let node_labels: Vec<String> = node_row.get("node_labels")?;
    let properties_json: String = node_row.get("properties_json")?;
    let edge_count: i64 = relation_row.get("edge_count")?;

    assert_eq!(node_kind, "capability");
    assert_eq!(title, "Projection consumer foundation");
    assert_eq!(status, "ACTIVE");
    assert_eq!(
        node_labels,
        vec!["projection".to_string(), "foundation".to_string()]
    );
    assert!(properties_json.contains("\"phase\":\"build\""));
    assert_eq!(edge_count, 1);

    Ok(())
}

async fn connect_with_retry(
    uri: String,
    user: &str,
    password: &str,
) -> Result<Graph, Box<dyn Error + Send + Sync>> {
    let mut last_error: Option<Box<dyn Error + Send + Sync>> = None;

    for _ in 0..30 {
        match Graph::new(&uri, user, password).await {
            Ok(graph) => return Ok(graph),
            Err(error) => {
                last_error = Some(Box::new(error));
                sleep(Duration::from_secs(1)).await;
            }
        }
    }

    Err(last_error.expect("at least one connection attempt should fail"))
}

async fn seed_projection(graph: &Graph) -> Result<(), Box<dyn Error + Send + Sync>> {
    graph.run(query("MATCH (n) DETACH DELETE n")).await?;

    graph
        .run(
            query(
                "
CREATE (pack:RoleContextPackProjection {
    case_id: $case_id,
    role: $role,
    latest_summary: $latest_summary,
    token_budget_hint: $token_budget_hint
})
CREATE (case_header:CaseHeaderProjection {
    case_id: $case_id,
    title: $case_title,
    summary: $case_summary,
    status: $case_status,
    created_at: $case_created_at,
    created_by: $case_created_by
})
CREATE (plan_header:PlanHeaderProjection {
    plan_id: $plan_id,
    revision: $plan_revision,
    status: $plan_status,
    work_items_total: $plan_work_items_total,
    work_items_completed: $plan_work_items_completed
})
CREATE (work_item_a:WorkItemProjection {
    work_item_id: $work_item_a_id,
    title: $work_item_a_title,
    summary: $work_item_a_summary,
    role: $work_item_a_role,
    phase: $work_item_a_phase,
    status: $work_item_a_status,
    dependency_ids: $work_item_a_dependency_ids,
    priority: $work_item_a_priority
})
CREATE (work_item_b:WorkItemProjection {
    work_item_id: $work_item_b_id,
    title: $work_item_b_title,
    summary: $work_item_b_summary,
    role: $work_item_b_role,
    phase: $work_item_b_phase,
    status: $work_item_b_status,
    dependency_ids: $work_item_b_dependency_ids,
    priority: $work_item_b_priority
})
CREATE (decision_a:DecisionProjection {
    decision_id: $decision_a_id,
    title: $decision_a_title,
    rationale: $decision_a_rationale,
    status: $decision_a_status,
    owner: $decision_a_owner,
    decided_at: $decision_a_decided_at
})
CREATE (decision_b:DecisionProjection {
    decision_id: $decision_b_id,
    title: $decision_b_title,
    rationale: $decision_b_rationale,
    status: $decision_b_status,
    owner: $decision_b_owner,
    decided_at: $decision_b_decided_at
})
CREATE (relation:DecisionRelationProjection {
    source_decision_id: $relation_source_decision_id,
    target_decision_id: $relation_target_decision_id,
    relation_type: $relation_type
})
CREATE (impact:TaskImpactProjection {
    decision_id: $impact_decision_id,
    work_item_id: $impact_work_item_id,
    title: $impact_title,
    impact_type: $impact_type
})
CREATE (milestone:MilestoneProjection {
    milestone_type: $milestone_type,
    description: $milestone_description,
    occurred_at: $milestone_occurred_at,
    actor: $milestone_actor
})
CREATE (pack)-[:HAS_CASE_HEADER]->(case_header)
CREATE (pack)-[:HAS_PLAN_HEADER]->(plan_header)
CREATE (pack)-[:INCLUDES_WORK_ITEM]->(work_item_a)
CREATE (pack)-[:INCLUDES_WORK_ITEM]->(work_item_b)
CREATE (pack)-[:INCLUDES_DECISION]->(decision_a)
CREATE (pack)-[:INCLUDES_DECISION]->(decision_b)
CREATE (pack)-[:HAS_DECISION_RELATION]->(relation)
CREATE (pack)-[:HAS_TASK_IMPACT]->(impact)
CREATE (pack)-[:HAS_MILESTONE]->(milestone)
            ",
            )
            .param("case_id", "case-123")
            .param("role", "developer")
            .param("latest_summary", "Prioritize the read path before replay")
            .param("token_budget_hint", 6_144_i64)
            .param("case_title", "Graph-backed case")
            .param("case_summary", "The first real Neo4j read model")
            .param("case_status", "ACTIVE")
            .param("case_created_at", 1_728_345_600_000_i64)
            .param("case_created_by", "planner")
            .param("plan_id", "plan-123")
            .param("plan_revision", 4_i64)
            .param("plan_status", "ACTIVE")
            .param("plan_work_items_total", 2_i64)
            .param("plan_work_items_completed", 1_i64)
            .param("work_item_a_id", "story-001")
            .param("work_item_a_title", "Seed the projection")
            .param(
                "work_item_a_summary",
                "Write deterministic projection nodes",
            )
            .param("work_item_a_role", "developer")
            .param("work_item_a_phase", "delivery")
            .param("work_item_a_status", "done")
            .param("work_item_a_dependency_ids", Vec::<String>::new())
            .param("work_item_a_priority", 50_i64)
            .param("work_item_b_id", "story-002")
            .param("work_item_b_title", "Read the projection")
            .param("work_item_b_summary", "Load a RoleContextPack from Neo4j")
            .param("work_item_b_role", "developer")
            .param("work_item_b_phase", "delivery")
            .param("work_item_b_status", "in_progress")
            .param("work_item_b_dependency_ids", vec!["story-001".to_string()])
            .param("work_item_b_priority", 90_i64)
            .param("decision_a_id", "decision-001")
            .param("decision_a_title", "Keep the projection internal")
            .param("decision_a_rationale", "The kernel owns its read model")
            .param("decision_a_status", "accepted")
            .param("decision_a_owner", "architect")
            .param("decision_a_decided_at", 1_728_345_610_000_i64)
            .param("decision_b_id", "decision-002")
            .param("decision_b_title", "Read from Neo4j before replay")
            .param(
                "decision_b_rationale",
                "Unblocks query path without waiting for consumers",
            )
            .param("decision_b_status", "accepted")
            .param("decision_b_owner", "lead")
            .param("decision_b_decided_at", 1_728_345_620_000_i64)
            .param("relation_source_decision_id", "decision-002")
            .param("relation_target_decision_id", "decision-001")
            .param("relation_type", "depends_on")
            .param("impact_decision_id", "decision-002")
            .param("impact_work_item_id", "story-002")
            .param("impact_title", "Read model must land before rollout")
            .param("impact_type", "blocks")
            .param("milestone_type", "phase_transition")
            .param("milestone_description", "Moved to delivery")
            .param("milestone_occurred_at", 1_728_345_630_000_i64)
            .param("milestone_actor", "planner"),
        )
        .await?;

    Ok(())
}

async fn single_row(
    graph: &Graph,
    query: neo4rs::Query,
) -> Result<neo4rs::Row, Box<dyn Error + Send + Sync>> {
    let mut rows = graph.execute(query).await?;
    rows.next()
        .await?
        .ok_or_else(|| "expected at least one row".into())
}
