use serde::Serialize;

use rehydration_application::UpdateContextOutcome;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct UpdateContextResponsePayload {
    pub story_id: String,
    pub status: String,
    pub version: i32,
    pub hash: String,
    pub warnings: Vec<String>,
}

impl UpdateContextResponsePayload {
    pub(crate) fn new(story_id: String, outcome: &UpdateContextOutcome) -> Self {
        Self {
            story_id,
            status: "success".to_string(),
            version: outcome.accepted_version.revision.min(i32::MAX as u64) as i32,
            hash: outcome.accepted_version.content_hash.clone(),
            warnings: outcome.warnings.clone(),
        }
    }
}
