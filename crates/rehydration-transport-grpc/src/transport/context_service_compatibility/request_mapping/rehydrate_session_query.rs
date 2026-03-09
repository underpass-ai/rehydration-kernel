use rehydration_application::RehydrateSessionQuery;
use rehydration_proto::fleet_context_v1::RehydrateSessionRequest;

const DEFAULT_TIMELINE_EVENTS: u32 = 50;
const DEFAULT_SNAPSHOT_TTL_SECONDS: u64 = 3600;

pub(crate) fn map_rehydrate_session_query(
    request: RehydrateSessionRequest,
) -> RehydrateSessionQuery {
    RehydrateSessionQuery {
        root_node_id: request.case_id,
        roles: request.roles,
        persist_snapshot: request.persist_bundle,
        snapshot_ttl_seconds: snapshot_ttl_seconds(request.ttl_seconds),
        timeline_window: timeline_window(request.timeline_events),
    }
}

fn timeline_window(value: i32) -> u32 {
    if value > 0 {
        value as u32
    } else {
        DEFAULT_TIMELINE_EVENTS
    }
}

fn snapshot_ttl_seconds(value: i32) -> u64 {
    if value > 0 {
        value as u64
    } else {
        DEFAULT_SNAPSHOT_TTL_SECONDS
    }
}

#[cfg(test)]
mod tests {
    use rehydration_proto::fleet_context_v1::RehydrateSessionRequest;

    use super::map_rehydrate_session_query;

    #[test]
    fn rehydrate_session_query_preserves_roles_and_applies_external_default_timeline() {
        let query = map_rehydrate_session_query(RehydrateSessionRequest {
            case_id: "case-123".to_string(),
            roles: vec!["DEV".to_string(), "QA".to_string()],
            include_timeline: true,
            include_summaries: true,
            timeline_events: -4,
            persist_bundle: true,
            ttl_seconds: 900,
        });

        assert_eq!(query.root_node_id, "case-123");
        assert_eq!(query.roles, vec!["DEV".to_string(), "QA".to_string()]);
        assert_eq!(query.timeline_window, 50);
        assert_eq!(query.snapshot_ttl_seconds, 900);
        assert!(query.persist_snapshot);
    }

    #[test]
    fn rehydrate_session_query_applies_external_default_snapshot_ttl() {
        let query = map_rehydrate_session_query(RehydrateSessionRequest {
            case_id: "case-123".to_string(),
            roles: vec!["DEV".to_string()],
            include_timeline: true,
            include_summaries: true,
            timeline_events: 4,
            persist_bundle: true,
            ttl_seconds: 0,
        });

        assert_eq!(query.snapshot_ttl_seconds, 3600);
        assert_eq!(query.timeline_window, 4);
    }
}
