use std::collections::BTreeMap;
use std::time::{Duration, SystemTime};

use rehydration_domain::{CaseId, Decision, Milestone, PlanHeader, WorkItem};
use rehydration_ports::{NodeDetailProjection, NodeProjection, PortError, ProjectionMutation};

use super::endpoint::{Neo4jEndpoint, parse_authority, parse_host_port, split_uri};
use super::projection_store::Neo4jProjectionStore;
use super::row_mapping::{
    ProjectionRootRecord, RawCaseHeaderRecord, RawDecisionRecord, RawMilestoneRecord,
    RawPlanHeaderRecord, RawProjectionRootRecord, RawWorkItemRecord, parse_system_time,
    serialize_properties,
};

#[test]
fn endpoint_supports_auth_segments() {
    let endpoint = Neo4jEndpoint::parse("neo4j://neo4j:neo@localhost:7687".to_string())
        .expect("uri should parse");

    assert_eq!(endpoint.connection_uri, "neo4j://localhost:7687");
    assert_eq!(endpoint.user, "neo4j");
    assert_eq!(endpoint.password, "neo");
}

#[test]
fn projection_store_keeps_endpoint_configuration() {
    let store =
        Neo4jProjectionStore::new("neo4j://neo4j:secret@localhost:7687").expect("store init");

    let rendered = format!("{store:?}");
    assert!(rendered.contains("Neo4jProjectionStore"));
    assert!(rendered.contains("connected: false"));
}

#[test]
fn endpoint_rejects_query_params_and_paths() {
    let with_query = Neo4jEndpoint::parse("neo4j://localhost:7687?db=neo4j".to_string())
        .expect_err("query params are not supported yet");
    let with_path = Neo4jEndpoint::parse("neo4j://localhost:7687/graph".to_string())
        .expect_err("paths are not supported");

    assert_eq!(
        with_query,
        PortError::InvalidState("graph uri query params are not supported yet".to_string())
    );
    assert_eq!(
        with_path,
        PortError::InvalidState("graph uri path segments are not supported".to_string())
    );
}

#[test]
fn parser_rejects_invalid_scheme() {
    let error = Neo4jEndpoint::parse("https://localhost:7687".to_string())
        .expect_err("unsupported schemes must fail");

    assert_eq!(
        error,
        PortError::InvalidState("unsupported graph scheme `https`".to_string())
    );
}

#[test]
fn parser_rejects_missing_scheme_and_host() {
    let missing_scheme =
        Neo4jEndpoint::parse("localhost:7687".to_string()).expect_err("scheme is required");
    let missing_host = Neo4jEndpoint::parse("neo4j://".to_string()).expect_err("host is required");

    assert_eq!(
        missing_scheme,
        PortError::InvalidState("graph uri must include a scheme".to_string())
    );
    assert_eq!(
        missing_host,
        PortError::InvalidState("graph uri must include a host".to_string())
    );
}

#[test]
fn parser_rejects_unsupported_authorities() {
    let missing_password =
        parse_authority("neo4j@localhost:7687", "graph").expect_err("auth must include password");
    let invalid_separator =
        parse_host_port("[::1]7687", "graph").expect_err("ipv6 port separator must be explicit");
    let invalid_port =
        parse_host_port("localhost:not-a-port", "graph").expect_err("port must be numeric");

    assert_eq!(
        missing_password,
        PortError::InvalidState(
            "graph uri auth segments must include username and password".to_string()
        )
    );
    assert_eq!(
        invalid_separator,
        PortError::InvalidState("graph uri contains an invalid port separator".to_string())
    );
    assert!(
        invalid_port
            .to_string()
            .starts_with("graph uri contains an invalid port:")
    );
}

#[test]
fn split_uri_supports_ipv6_without_losing_authority() {
    let uri = split_uri("neo4j://[::1]:7687", "graph").expect("uri should parse");
    parse_host_port(uri.authority, "graph").expect("ipv6 authority should be valid");

    assert_eq!(uri.scheme, "neo4j");
    assert_eq!(uri.authority, "[::1]:7687");
    assert!(uri.query.is_none());
}

#[test]
fn root_record_requires_non_negative_token_budget() {
    let error = ProjectionRootRecord::try_from(RawProjectionRootRecord {
        latest_summary: "latest".to_string(),
        token_budget_hint: -1,
    })
    .expect_err("negative token budget must fail");

    assert_eq!(
        error,
        PortError::InvalidState(
            "neo4j projection field `token_budget_hint` must be a non-negative u32".to_string()
        )
    );
}

#[test]
fn raw_case_header_maps_to_domain() {
    let created_at = 1_728_345_600_000_i64;
    let header = rehydration_domain::CaseHeader::try_from(RawCaseHeaderRecord {
        case_id: "case-123".to_string(),
        title: "Graph-backed case".to_string(),
        summary: "Loaded from neo4j".to_string(),
        status: "ACTIVE".to_string(),
        created_at_millis: created_at,
        created_by: "planner".to_string(),
    })
    .expect("record should map");

    assert_eq!(
        header.case_id(),
        &CaseId::new("case-123").expect("case id is valid")
    );
    assert_eq!(header.title(), "Graph-backed case");
    assert_eq!(
        header.created_at(),
        SystemTime::UNIX_EPOCH + Duration::from_millis(created_at as u64)
    );
}

#[test]
fn raw_plan_header_maps_to_domain() {
    let plan = PlanHeader::try_from(RawPlanHeaderRecord {
        plan_id: "plan-123".to_string(),
        revision: 7,
        status: "ACTIVE".to_string(),
        work_items_total: 10,
        work_items_completed: 4,
    })
    .expect("record should map");

    assert_eq!(plan.plan_id(), "plan-123");
    assert_eq!(plan.revision(), 7);
    assert_eq!(plan.work_items_total(), 10);
}

#[test]
fn raw_work_item_maps_to_domain() {
    let work_item = WorkItem::try_from(RawWorkItemRecord {
        work_item_id: "story-123".to_string(),
        title: "Implement read model".to_string(),
        summary: "Port the first real query".to_string(),
        role: "developer".to_string(),
        phase: "delivery".to_string(),
        status: "in_progress".to_string(),
        dependency_ids: vec!["story-100".to_string()],
        priority: 90,
    })
    .expect("record should map");

    assert_eq!(work_item.work_item_id(), "story-123");
    assert_eq!(work_item.dependency_ids(), &["story-100".to_string()]);
    assert_eq!(work_item.priority(), 90);
}

#[test]
fn raw_decision_and_milestone_map_to_domain() {
    let decision = Decision::try_from(RawDecisionRecord {
        decision_id: "dec-1".to_string(),
        title: "Use Neo4j projection".to_string(),
        rationale: "Supports graph traversal later".to_string(),
        status: "accepted".to_string(),
        owner: "architect".to_string(),
        decided_at_millis: 10,
    })
    .expect("decision should map");
    let milestone = Milestone::try_from(RawMilestoneRecord {
        milestone_type: "phase_transition".to_string(),
        description: "Moved to delivery".to_string(),
        occurred_at_millis: 20,
        actor: "planner".to_string(),
    })
    .expect("milestone should map");

    assert_eq!(decision.decision_id(), "dec-1");
    assert_eq!(decision.owner(), "architect");
    assert_eq!(milestone.milestone_type(), "phase_transition");
    assert_eq!(milestone.actor(), "planner");
}

#[test]
fn timestamps_must_be_non_negative() {
    let error =
        parse_system_time(-1, "case_header.created_at").expect_err("negative timestamps must fail");

    assert_eq!(
        error,
        PortError::InvalidState(
            "neo4j projection field `case_header.created_at` must be a non-negative unix timestamp in milliseconds".to_string()
        )
    );
}

#[test]
fn serialize_properties_emits_json_object() {
    let properties = BTreeMap::from([
        ("phase".to_string(), "build".to_string()),
        ("role".to_string(), "developer".to_string()),
    ]);

    let serialized = serialize_properties(&properties).expect("properties should serialize");
    let parsed: serde_json::Value = serde_json::from_str(&serialized).expect("json should parse");

    assert_eq!(parsed["phase"], serde_json::json!("build"));
    assert_eq!(parsed["role"], serde_json::json!("developer"));
}

#[test]
fn node_detail_mutation_variant_is_reserved_for_valkey() {
    let mutation = ProjectionMutation::UpsertNodeDetail(NodeDetailProjection {
        node_id: "node-123".to_string(),
        detail: "expanded".to_string(),
        content_hash: "hash-123".to_string(),
        revision: 1,
    });

    match mutation {
        ProjectionMutation::UpsertNodeDetail(detail) => {
            assert_eq!(detail.node_id, "node-123");
        }
        other => panic!("unexpected mutation: {other:?}"),
    }
}

#[test]
fn node_projection_properties_can_be_prepared_for_persistence() {
    let projection = NodeProjection {
        node_id: "node-123".to_string(),
        node_kind: "capability".to_string(),
        title: "Projection foundation".to_string(),
        summary: "Node centric".to_string(),
        status: "ACTIVE".to_string(),
        labels: vec!["projection".to_string()],
        properties: BTreeMap::from([("phase".to_string(), "build".to_string())]),
    };

    let serialized =
        serialize_properties(&projection.properties).expect("properties should serialize");
    assert!(serialized.contains("\"phase\":\"build\""));
}
