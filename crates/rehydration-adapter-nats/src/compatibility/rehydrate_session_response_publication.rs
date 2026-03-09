use crate::NatsConsumerError;
use crate::compatibility::event_envelope_factory::create_event_envelope;
use crate::compatibility::publication::NatsPublication;
use crate::compatibility::rehydrate_session_response_payload::RehydrateSessionResponsePayload;
use rehydration_application::RehydrateSessionResult;

pub(crate) fn build_rehydrate_session_response_publication(
    result: &RehydrateSessionResult,
) -> Result<NatsPublication, NatsConsumerError> {
    let payload = RehydrateSessionResponsePayload::new(result);
    let envelope = create_event_envelope(
        "context.rehydrate.response",
        &payload,
        "context-service",
        &result.root_node_id,
        Some("rehydrate_response"),
    )?;
    let payload = serde_json::to_vec(&envelope).map_err(|error| {
        NatsConsumerError::Publish(format!(
            "failed to serialize rehydrate session response envelope: {error}"
        ))
    })?;

    Ok(NatsPublication {
        subject: "context.rehydrate.response".to_string(),
        payload,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::Value;

    use rehydration_application::RehydrateSessionResult;
    use rehydration_domain::{
        BundleMetadata, BundleNode, BundleNodeDetail, CaseId, RehydrationBundle, Role,
    };

    use super::build_rehydrate_session_response_publication;

    #[test]
    fn rehydrate_response_publication_uses_frozen_subject_and_summary_payload() {
        let bundle = RehydrationBundle::new(
            CaseId::new("case-1").expect("case id"),
            Role::new("developer").expect("role"),
            BundleNode::new(
                "case-1",
                "story",
                "Story",
                "Summary",
                "ACTIVE",
                vec!["Story".to_string()],
                BTreeMap::new(),
            ),
            Vec::new(),
            Vec::new(),
            vec![BundleNodeDetail::new("case-1", "detail", "hash-1", 1)],
            BundleMetadata::initial("0.1.0"),
        )
        .expect("bundle");

        let publication = build_rehydrate_session_response_publication(&RehydrateSessionResult {
            root_node_id: "case-1".to_string(),
            bundles: vec![bundle],
            timeline_events: 50,
            version: BundleMetadata::initial("0.1.0"),
            snapshot_persisted: false,
            snapshot_id: None,
            generated_at: std::time::SystemTime::UNIX_EPOCH,
        })
        .expect("publication should serialize");

        assert_eq!(publication.subject, "context.rehydrate.response");
        let envelope: Value =
            serde_json::from_slice(&publication.payload).expect("payload must be json");
        assert_eq!(envelope["event_type"], "context.rehydrate.response");
        assert_eq!(envelope["payload"]["case_id"], "case-1");
        assert_eq!(envelope["payload"]["packs_count"], 1);
        assert_eq!(envelope["payload"]["stats"]["events"], 50);
    }
}
