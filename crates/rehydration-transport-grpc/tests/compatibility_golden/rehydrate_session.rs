use std::error::Error;

use rehydration_proto::fleet_context_v1::RehydrateSessionRequest;

use crate::support::golden_contract::expected_rehydrate_session_response;
use crate::support::seed_data::{DEVELOPER_ROLE, REHYDRATE_TIMELINE_EVENTS, ROOT_NODE_ID};
use crate::support::seeded_fixture::SeededCompatibilityFixture;

#[tokio::test]
async fn grpc_rehydrate_session_basic_matches_golden_contract()
-> Result<(), Box<dyn Error + Send + Sync>> {
    let mut fixture = SeededCompatibilityFixture::start().await?;

    let result = async {
        let mut response = fixture
            .client()
            .rehydrate_session(RehydrateSessionRequest {
                case_id: ROOT_NODE_ID.to_string(),
                roles: vec![DEVELOPER_ROLE.to_string()],
                include_timeline: true,
                include_summaries: true,
                timeline_events: REHYDRATE_TIMELINE_EVENTS,
                persist_bundle: false,
                ttl_seconds: 120,
            })
            .await?
            .into_inner();

        assert!(response.generated_at_ms > 0);
        response.generated_at_ms = 0;
        assert_eq!(
            response,
            expected_rehydrate_session_response(0, REHYDRATE_TIMELINE_EVENTS)
        );
        Ok::<(), Box<dyn Error + Send + Sync>>(())
    }
    .await;

    fixture.shutdown().await?;
    result
}

#[tokio::test]
async fn grpc_rehydrate_session_defaults_timeline_events_to_frozen_external_value()
-> Result<(), Box<dyn Error + Send + Sync>> {
    let mut fixture = SeededCompatibilityFixture::start().await?;

    let result = async {
        let mut response = fixture
            .client()
            .rehydrate_session(RehydrateSessionRequest {
                case_id: ROOT_NODE_ID.to_string(),
                roles: vec![DEVELOPER_ROLE.to_string()],
                include_timeline: true,
                include_summaries: true,
                timeline_events: 0,
                persist_bundle: false,
                ttl_seconds: 120,
            })
            .await?
            .into_inner();

        assert!(response.generated_at_ms > 0);
        response.generated_at_ms = 0;
        assert_eq!(response, expected_rehydrate_session_response(0, 50));
        Ok::<(), Box<dyn Error + Send + Sync>>(())
    }
    .await;

    fixture.shutdown().await?;
    result
}
