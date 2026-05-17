use std::collections::BTreeMap;
use std::collections::BTreeSet;

use crate::{
    DomainError, DomainResult, ExampleCount, KernelTool, OperatorAction, OperatorMode,
    PositiveCount, TrainingTrajectory,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrainingDatasetPreflightReport {
    total: PositiveCount,
    by_mode: BTreeMap<OperatorMode, ExampleCount>,
    by_target_tool: BTreeMap<KernelTool, ExampleCount>,
    stop_actions: ExampleCount,
}

impl TrainingDatasetPreflightReport {
    pub fn from_trajectories(trajectories: &[TrainingTrajectory]) -> DomainResult<Self> {
        if trajectories.is_empty() {
            return Err(DomainError::EmptyCollection {
                context: "training_dataset.trajectories".to_string(),
            });
        }

        let mut seen_step_ids = BTreeSet::new();
        let mut by_mode = BTreeMap::<OperatorMode, usize>::new();
        let mut by_target_tool = BTreeMap::<KernelTool, usize>::new();
        let mut stop_actions = 0usize;

        for trajectory in trajectories {
            let step_id = trajectory.step_id().as_str().to_string();
            if !seen_step_ids.insert(step_id.clone()) {
                return Err(DomainError::DuplicateStepId { step_id });
            }

            *by_mode.entry(trajectory.mode()).or_default() += 1;
            match trajectory.target_action() {
                OperatorAction::ToolCall(action) => {
                    *by_target_tool.entry(action.tool()).or_default() += 1;
                }
                OperatorAction::PreparedToolCall { tool, .. } => {
                    *by_target_tool.entry(*tool).or_default() += 1;
                }
                OperatorAction::Stop(_) => {
                    stop_actions += 1;
                }
            }
        }

        Self::new(
            PositiveCount::parse(trajectories.len(), "training_dataset.total")?,
            map_counts(by_mode),
            map_counts(by_target_tool),
            ExampleCount::from_usize(stop_actions),
        )
    }

    pub fn new(
        total: PositiveCount,
        by_mode: BTreeMap<OperatorMode, ExampleCount>,
        by_target_tool: BTreeMap<KernelTool, ExampleCount>,
        stop_actions: ExampleCount,
    ) -> DomainResult<Self> {
        Ok(Self {
            total,
            by_mode,
            by_target_tool,
            stop_actions,
        })
    }

    pub fn total(&self) -> PositiveCount {
        self.total
    }

    pub fn by_mode(&self) -> &BTreeMap<OperatorMode, ExampleCount> {
        &self.by_mode
    }

    pub fn by_target_tool(&self) -> &BTreeMap<KernelTool, ExampleCount> {
        &self.by_target_tool
    }

    pub fn stop_actions(&self) -> ExampleCount {
        self.stop_actions
    }
}

fn map_counts<K>(counts: BTreeMap<K, usize>) -> BTreeMap<K, ExampleCount>
where
    K: Ord,
{
    counts
        .into_iter()
        .map(|(key, count)| (key, ExampleCount::from_usize(count)))
        .collect()
}
