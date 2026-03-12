use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct StarshipDemoProviderSummary {
    pub llm_provider: String,
    pub runtime_mode: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StarshipDemoPhaseSummary {
    pub step_node_id: String,
    pub written_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StarshipDemoSummary {
    pub run_id: String,
    pub root_node_id: String,
    pub workspace_dir: String,
    pub provider: StarshipDemoProviderSummary,
    pub phase_one: StarshipDemoPhaseSummary,
    pub phase_two: StarshipDemoPhaseSummary,
    pub captains_log: Option<String>,
}
