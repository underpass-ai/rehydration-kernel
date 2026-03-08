use rehydration_application::{
    GetProjectionStatusResult, ProjectionStatusView, ReplayProjectionOutcome,
};
use rehydration_proto::v1alpha1::{
    GetProjectionStatusResponse, ProjectionStatus, ReplayProjectionResponse,
};

use crate::transport::support::{proto_replay_mode, timestamp_from};

pub(crate) fn proto_projection_status_response(
    result: &GetProjectionStatusResult,
) -> GetProjectionStatusResponse {
    GetProjectionStatusResponse {
        projections: result
            .projections
            .iter()
            .map(proto_projection_status)
            .collect(),
        observed_at: Some(timestamp_from(result.observed_at)),
    }
}

pub(crate) fn proto_replay_projection_response(
    result: &ReplayProjectionOutcome,
) -> ReplayProjectionResponse {
    ReplayProjectionResponse {
        replay_id: result.replay_id.clone(),
        consumer_name: result.consumer_name.clone(),
        replay_mode: proto_replay_mode(result.replay_mode) as i32,
        accepted_events: result.accepted_events,
        requested_at: Some(timestamp_from(result.requested_at)),
    }
}

pub(crate) fn proto_projection_status(view: &ProjectionStatusView) -> ProjectionStatus {
    ProjectionStatus {
        consumer_name: view.consumer_name.clone(),
        stream_name: view.stream_name.clone(),
        projection_watermark: view.projection_watermark.clone(),
        processed_events: view.processed_events,
        pending_events: view.pending_events,
        last_event_at: Some(timestamp_from(view.last_event_at)),
        updated_at: Some(timestamp_from(view.updated_at)),
        healthy: view.healthy,
        warnings: view.warnings.clone(),
    }
}
