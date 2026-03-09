use serde::Deserialize;

use crate::compatibility::update_context_change_payload::UpdateContextChangePayload;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub(crate) struct UpdateContextRequestPayload {
    #[serde(default)]
    pub story_id: String,
    #[serde(default)]
    pub task_id: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub changes: Vec<UpdateContextChangePayload>,
    #[serde(default)]
    pub timestamp: String,
}
