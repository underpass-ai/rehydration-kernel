use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct ContextUpdatedEventPayload {
    pub story_id: String,
    pub version: u64,
    pub timestamp: f64,
}

impl ContextUpdatedEventPayload {
    pub(crate) fn new(story_id: impl Into<String>, version: u64, timestamp: f64) -> Self {
        Self {
            story_id: story_id.into(),
            version,
            timestamp,
        }
    }
}
