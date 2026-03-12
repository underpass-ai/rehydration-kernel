#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmStarshipMissionExecution {
    pub selected_step_node_id: String,
    pub written_paths: Vec<String>,
    pub captains_log: Option<String>,
}
