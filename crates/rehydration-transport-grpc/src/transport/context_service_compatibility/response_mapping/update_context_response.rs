use rehydration_application::UpdateContextOutcome;
use rehydration_proto::fleet_context_v1::UpdateContextResponse;

pub(crate) fn proto_update_context_response(
    outcome: &UpdateContextOutcome,
) -> UpdateContextResponse {
    UpdateContextResponse {
        version: outcome.accepted_version.revision.min(i32::MAX as u64) as i32,
        hash: outcome.accepted_version.content_hash.clone(),
        warnings: outcome.warnings.clone(),
    }
}

#[cfg(test)]
mod tests {
    use rehydration_application::{AcceptedVersion, UpdateContextOutcome};

    use super::proto_update_context_response;

    #[test]
    fn update_context_response_uses_external_field_names() {
        let response = proto_update_context_response(&UpdateContextOutcome {
            accepted_version: AcceptedVersion {
                revision: 4,
                content_hash: "hash-4".to_string(),
                generator_version: "0.1.0".to_string(),
            },
            warnings: vec!["warn".to_string()],
            snapshot_persisted: false,
            snapshot_id: None,
        });

        assert_eq!(response.version, 4);
        assert_eq!(response.hash, "hash-4");
        assert_eq!(response.warnings, vec!["warn".to_string()]);
    }
}
