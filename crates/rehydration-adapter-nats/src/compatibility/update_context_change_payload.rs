use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub(crate) struct UpdateContextChangePayload {
    #[serde(default)]
    pub operation: String,
    #[serde(default)]
    pub entity_type: String,
    #[serde(default)]
    pub entity_id: String,
    #[serde(default)]
    pub payload: Option<Value>,
    #[serde(default)]
    pub reason: String,
}
