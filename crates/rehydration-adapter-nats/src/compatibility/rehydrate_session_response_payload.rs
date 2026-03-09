use serde::Serialize;

use rehydration_application::RehydrateSessionResult;

use crate::compatibility::rehydration_stats_payload::RehydrationStatsPayload;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct RehydrateSessionResponsePayload {
    pub case_id: String,
    pub status: String,
    pub generated_at_ms: i64,
    pub packs_count: i32,
    pub stats: RehydrationStatsPayload,
}

impl RehydrateSessionResponsePayload {
    pub(crate) fn new(result: &RehydrateSessionResult) -> Self {
        Self {
            case_id: result.root_node_id.clone(),
            status: "success".to_string(),
            generated_at_ms: millis_since_epoch(result.generated_at),
            packs_count: result.bundles.len().min(i32::MAX as usize) as i32,
            stats: RehydrationStatsPayload {
                decisions: result
                    .bundles
                    .iter()
                    .flat_map(|bundle| bundle.neighbor_nodes())
                    .filter(|node| node.node_kind().eq_ignore_ascii_case("decision"))
                    .count()
                    .min(i32::MAX as usize) as i32,
                decision_edges: result
                    .bundles
                    .iter()
                    .flat_map(|bundle| bundle.relationships())
                    .filter(|relationship| relationship.relationship_type().contains("DECISION"))
                    .count()
                    .min(i32::MAX as usize) as i32,
                impacts: result
                    .bundles
                    .iter()
                    .flat_map(|bundle| bundle.relationships())
                    .filter(|relationship| relationship.relationship_type().contains("IMPACT"))
                    .count()
                    .min(i32::MAX as usize) as i32,
                events: result.timeline_events.min(i32::MAX as u32) as i32,
                roles: result
                    .bundles
                    .iter()
                    .map(|bundle| bundle.role().as_str().to_string())
                    .collect(),
            },
        }
    }
}

fn millis_since_epoch(value: std::time::SystemTime) -> i64 {
    use std::time::UNIX_EPOCH;

    value
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}
