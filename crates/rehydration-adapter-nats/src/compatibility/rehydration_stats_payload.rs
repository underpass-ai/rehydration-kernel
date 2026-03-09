use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct RehydrationStatsPayload {
    pub decisions: i32,
    pub decision_edges: i32,
    pub impacts: i32,
    pub events: i32,
    pub roles: Vec<String>,
}
