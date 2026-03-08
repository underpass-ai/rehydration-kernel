#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayModeSelection {
    DryRun,
    Rebuild,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayProjectionCommand {
    pub consumer_name: String,
    pub stream_name: String,
    pub starting_after: Option<String>,
    pub max_events: u32,
    pub replay_mode: ReplayModeSelection,
    pub requested_by: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayProjectionOutcome {
    pub replay_id: String,
    pub consumer_name: String,
    pub replay_mode: ReplayModeSelection,
    pub accepted_events: u32,
    pub requested_at: std::time::SystemTime,
}
