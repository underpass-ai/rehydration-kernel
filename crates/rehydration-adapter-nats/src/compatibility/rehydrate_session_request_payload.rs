use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct RehydrateSessionRequestPayload {
    #[serde(default)]
    pub case_id: String,
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default)]
    pub include_timeline: bool,
    #[serde(default)]
    pub include_summaries: bool,
    #[serde(default = "default_timeline_events")]
    pub timeline_events: i32,
    #[serde(default)]
    pub persist_bundle: bool,
    #[serde(default = "default_ttl_seconds")]
    pub ttl_seconds: i32,
}

const fn default_timeline_events() -> i32 {
    50
}

const fn default_ttl_seconds() -> i32 {
    3600
}
