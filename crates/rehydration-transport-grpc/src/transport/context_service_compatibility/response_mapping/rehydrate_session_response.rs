use std::collections::HashMap;
use std::time::UNIX_EPOCH;

use rehydration_application::RehydrateSessionResult;
use rehydration_proto::fleet_context_v1::{RehydrateSessionResponse, RehydrationStats};

use crate::transport::context_service_compatibility::response_mapping::role_context_pack::proto_role_context_pack;

pub(crate) fn proto_rehydrate_session_response(
    result: &RehydrateSessionResult,
) -> RehydrateSessionResponse {
    RehydrateSessionResponse {
        case_id: result.root_node_id.clone(),
        generated_at_ms: millis_since_epoch(result.generated_at),
        packs: result
            .bundles
            .iter()
            .map(|bundle| {
                (
                    bundle.role().as_str().to_string(),
                    proto_role_context_pack(bundle),
                )
            })
            .collect::<HashMap<_, _>>(),
        stats: Some(proto_rehydration_stats(result)),
    }
}

fn proto_rehydration_stats(result: &RehydrateSessionResult) -> RehydrationStats {
    let decisions = result
        .bundles
        .iter()
        .flat_map(|bundle| bundle.neighbor_nodes())
        .filter(|node| node.node_kind().eq_ignore_ascii_case("decision"))
        .count();
    let decision_edges = result
        .bundles
        .iter()
        .flat_map(|bundle| bundle.relationships())
        .filter(|relationship| relationship.relationship_type().contains("DECISION"))
        .count();
    let impacts = result
        .bundles
        .iter()
        .flat_map(|bundle| bundle.relationships())
        .filter(|relationship| relationship.relationship_type().contains("IMPACT"))
        .count();

    RehydrationStats {
        decisions: decisions.min(i32::MAX as usize) as i32,
        decision_edges: decision_edges.min(i32::MAX as usize) as i32,
        impacts: impacts.min(i32::MAX as usize) as i32,
        events: result.timeline_events.min(i32::MAX as u32) as i32,
        roles: result
            .bundles
            .iter()
            .map(|bundle| bundle.role().as_str().to_string())
            .collect(),
    }
}

fn millis_since_epoch(value: std::time::SystemTime) -> i64 {
    value
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::time::{Duration, SystemTime};

    use rehydration_application::RehydrateSessionResult;
    use rehydration_domain::{
        BundleMetadata, BundleNode, BundleNodeDetail, BundleRelationship, CaseId,
        RehydrationBundle, Role,
    };

    use super::proto_rehydrate_session_response;

    #[test]
    fn rehydrate_session_response_builds_pack_map_and_stats() {
        let bundle = RehydrationBundle::new(
            CaseId::new("case-123").expect("case id"),
            Role::new("developer").expect("role"),
            BundleNode::new(
                "case-123",
                "story",
                "Story",
                "Summary",
                "ACTIVE",
                vec!["Story".to_string()],
                BTreeMap::new(),
            ),
            vec![
                BundleNode::new(
                    "task-1",
                    "task",
                    "Task",
                    "Task summary",
                    "ACTIVE",
                    vec!["Task".to_string()],
                    BTreeMap::new(),
                ),
                BundleNode::new(
                    "decision-1",
                    "decision",
                    "Decision",
                    "Decision summary",
                    "ACTIVE",
                    vec!["Decision".to_string()],
                    BTreeMap::new(),
                ),
            ],
            vec![BundleRelationship::new(
                "decision-1",
                "task-1",
                "IMPACTS",
                BTreeMap::new(),
            )],
            vec![BundleNodeDetail::new(
                "case-123",
                "Case detail",
                "hash-1",
                1,
            )],
            BundleMetadata::initial("0.1.0"),
        )
        .expect("bundle");

        let response = proto_rehydrate_session_response(&RehydrateSessionResult {
            root_node_id: "case-123".to_string(),
            bundles: vec![bundle],
            timeline_events: 7,
            version: BundleMetadata::initial("0.1.0"),
            snapshot_persisted: false,
            snapshot_id: None,
            generated_at: SystemTime::UNIX_EPOCH + Duration::from_secs(42),
        });

        assert_eq!(response.case_id, "case-123");
        assert!(response.packs.contains_key("developer"));
        assert_eq!(response.stats.expect("stats").events, 7);
    }
}
