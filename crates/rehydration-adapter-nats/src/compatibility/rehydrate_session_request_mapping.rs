use rehydration_application::RehydrateSessionQuery;

use crate::NatsConsumerError;
use crate::compatibility::rehydrate_session_request_payload::RehydrateSessionRequestPayload;

pub(crate) fn map_rehydrate_session_query(
    payload: RehydrateSessionRequestPayload,
) -> Result<RehydrateSessionQuery, NatsConsumerError> {
    if payload.case_id.trim().is_empty() {
        return Err(NatsConsumerError::InvalidRequest(
            "case_id is required in rehydrate session request".to_string(),
        ));
    }

    Ok(RehydrateSessionQuery {
        root_node_id: payload.case_id,
        roles: payload.roles,
        persist_snapshot: payload.persist_bundle,
        timeline_window: positive_or_default(payload.timeline_events, 50),
        snapshot_ttl_seconds: positive_or_default(payload.ttl_seconds, 3600) as u64,
    })
}

fn positive_or_default(value: i32, default: i32) -> u32 {
    if value > 0 {
        value as u32
    } else {
        default as u32
    }
}
