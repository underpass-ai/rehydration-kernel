use rehydration_application::RehydrateSessionQuery;
use rehydration_proto::fleet_context_v1::RehydrateSessionRequest;

pub(crate) fn map_rehydrate_session_query(
    request: RehydrateSessionRequest,
) -> RehydrateSessionQuery {
    RehydrateSessionQuery {
        root_node_id: request.case_id,
        roles: request.roles,
        persist_snapshot: request.persist_bundle,
        timeline_window: request.timeline_events.max(0) as u32,
    }
}

#[cfg(test)]
mod tests {
    use rehydration_proto::fleet_context_v1::RehydrateSessionRequest;

    use super::map_rehydrate_session_query;

    #[test]
    fn rehydrate_session_query_preserves_roles_and_clamps_negative_timeline() {
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
        assert_eq!(query.timeline_window, 0);
        assert!(query.persist_snapshot);
    }
}
