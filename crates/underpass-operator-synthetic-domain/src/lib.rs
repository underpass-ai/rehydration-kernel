use underpass_operator_shared_domain::{
    AboutId, AllowedTools, DatasetId, DomainError, DomainResult, ExampleCount, KernelTool,
    KmpMcpCapability, OperatorAction, OperatorMode, PositiveCount, StepId, SyntheticCaseId,
    TaskFamily, TrainingTrajectory, VisibleState,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntheticCaseSpec {
    case_id: SyntheticCaseId,
    mode: OperatorMode,
    task_family: TaskFamily,
    target_tool: KernelTool,
    minimum_examples: PositiveCount,
}

impl SyntheticCaseSpec {
    pub fn new(
        case_id: SyntheticCaseId,
        mode: OperatorMode,
        task_family: TaskFamily,
        target_tool: KernelTool,
        minimum_examples: PositiveCount,
    ) -> DomainResult<Self> {
        if !mode.allows_tool(target_tool) {
            return Err(DomainError::ToolOutsideMode {
                mode: mode.as_str().to_string(),
                tool: target_tool.as_str().to_string(),
            });
        }

        Ok(Self {
            case_id,
            mode,
            task_family,
            target_tool,
            minimum_examples,
        })
    }

    pub fn case_id(&self) -> &SyntheticCaseId {
        &self.case_id
    }

    pub fn mode(&self) -> OperatorMode {
        self.mode
    }

    pub fn task_family(&self) -> &TaskFamily {
        &self.task_family
    }

    pub fn target_tool(&self) -> KernelTool {
        self.target_tool
    }

    pub fn minimum_examples(&self) -> PositiveCount {
        self.minimum_examples
    }

    pub fn build_trajectory(
        &self,
        spec: OperatorTrajectoryBuildSpec,
    ) -> DomainResult<TrainingTrajectory> {
        self.validate_target_action(&spec.target_action)?;
        TrainingTrajectory::new(
            spec.step_id,
            spec.about,
            self.mode,
            self.task_family.clone(),
            AllowedTools::all_for_mode(self.mode),
            spec.visible_state,
            spec.target_action,
        )
    }

    pub fn validate_trajectory(&self, trajectory: &TrainingTrajectory) -> DomainResult<()> {
        if trajectory.mode() != self.mode {
            return Err(DomainError::TrajectoryCaseMismatch {
                field: "mode".to_string(),
                expected: self.mode.as_str().to_string(),
                actual: trajectory.mode().as_str().to_string(),
            });
        }
        if trajectory.task_family() != &self.task_family {
            return Err(DomainError::TrajectoryCaseMismatch {
                field: "task_family".to_string(),
                expected: self.task_family.as_str().to_string(),
                actual: trajectory.task_family().as_str().to_string(),
            });
        }
        if trajectory.allowed_tools().mode() != self.mode {
            return Err(DomainError::TrajectoryCaseMismatch {
                field: "allowed_tools.mode".to_string(),
                expected: self.mode.as_str().to_string(),
                actual: trajectory.allowed_tools().mode().as_str().to_string(),
            });
        }
        let expected_tools = AllowedTools::all_for_mode(self.mode);
        if trajectory.allowed_tools().as_slice() != expected_tools.as_slice() {
            return Err(DomainError::TrajectoryCaseMismatch {
                field: "allowed_tools".to_string(),
                expected: tool_list(expected_tools.as_slice()),
                actual: tool_list(trajectory.allowed_tools().as_slice()),
            });
        }
        self.validate_target_action(trajectory.target_action())
    }

    pub fn generation_metric(
        &self,
        generated: ExampleCount,
    ) -> DomainResult<SyntheticCaseGenerationMetric> {
        let metric = SyntheticCaseGenerationMetric::new(
            self.case_id.clone(),
            self.minimum_examples,
            generated,
        );
        if !metric.satisfies_minimum() {
            return Err(DomainError::CountBelowMinimum {
                context: format!("synthetic_case.{}.generated", self.case_id.as_str()),
                minimum: self.minimum_examples.as_usize(),
                actual: generated.as_usize(),
            });
        }

        Ok(metric)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntheticDatasetBlueprint {
    dataset_id: DatasetId,
    cases: Vec<SyntheticCaseSpec>,
}

impl SyntheticDatasetBlueprint {
    pub fn new(dataset_id: DatasetId, cases: Vec<SyntheticCaseSpec>) -> DomainResult<Self> {
        if cases.is_empty() {
            return Err(DomainError::EmptyCollection {
                context: "synthetic_dataset_blueprint.cases".to_string(),
            });
        }

        Ok(Self { dataset_id, cases })
    }

    pub fn for_kmp_mcp_capabilities(
        dataset_id: DatasetId,
        capabilities: &[KmpMcpCapability],
        minimum_examples_per_capability: PositiveCount,
    ) -> DomainResult<Self> {
        if capabilities.is_empty() {
            return Err(DomainError::EmptyCollection {
                context: "synthetic_dataset_blueprint.capabilities".to_string(),
            });
        }

        let cases = capabilities
            .iter()
            .copied()
            .map(|capability| {
                SyntheticCaseSpec::new(
                    SyntheticCaseId::parse(format!("kmp_mcp:{}", capability.name()))?,
                    capability.mode(),
                    TaskFamily::parse(format!("contract.{}", capability.name()))?,
                    capability.tool(),
                    minimum_examples_per_capability,
                )
            })
            .collect::<DomainResult<Vec<_>>>()?;

        Self::new(dataset_id, cases)
    }

    pub fn dataset_id(&self) -> &DatasetId {
        &self.dataset_id
    }

    pub fn cases(&self) -> &[SyntheticCaseSpec] {
        &self.cases
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct OperatorTrajectoryBuildSpec {
    step_id: StepId,
    about: AboutId,
    visible_state: VisibleState,
    target_action: OperatorAction,
}

impl OperatorTrajectoryBuildSpec {
    pub fn new(
        step_id: StepId,
        about: AboutId,
        visible_state: VisibleState,
        target_action: OperatorAction,
    ) -> Self {
        Self {
            step_id,
            about,
            visible_state,
            target_action,
        }
    }
}

impl SyntheticCaseSpec {
    fn validate_target_action(&self, action: &OperatorAction) -> DomainResult<()> {
        let actual = action.tool().map(KernelTool::as_str).unwrap_or("stop");
        if actual != self.target_tool.as_str() {
            return Err(DomainError::TrajectoryCaseMismatch {
                field: "target_action.tool".to_string(),
                expected: self.target_tool.as_str().to_string(),
                actual: actual.to_string(),
            });
        }
        Ok(())
    }
}

fn tool_list(tools: &[KernelTool]) -> String {
    tools
        .iter()
        .copied()
        .map(KernelTool::as_str)
        .collect::<Vec<_>>()
        .join(",")
}

#[derive(Debug, Clone, PartialEq)]
pub struct SyntheticDataset {
    dataset_id: DatasetId,
    trajectories: Vec<TrainingTrajectory>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntheticCaseGenerationMetric {
    case_id: SyntheticCaseId,
    expected_minimum: PositiveCount,
    generated: ExampleCount,
}

impl SyntheticCaseGenerationMetric {
    pub fn new(
        case_id: SyntheticCaseId,
        expected_minimum: PositiveCount,
        generated: ExampleCount,
    ) -> Self {
        Self {
            case_id,
            expected_minimum,
            generated,
        }
    }

    pub fn case_id(&self) -> &SyntheticCaseId {
        &self.case_id
    }

    pub fn expected_minimum(&self) -> PositiveCount {
        self.expected_minimum
    }

    pub fn generated(&self) -> ExampleCount {
        self.generated
    }

    pub fn satisfies_minimum(&self) -> bool {
        self.generated.as_usize() >= self.expected_minimum.as_usize()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SyntheticDatasetGenerationReport {
    dataset: SyntheticDataset,
    case_metrics: Vec<SyntheticCaseGenerationMetric>,
}

impl SyntheticDatasetGenerationReport {
    pub fn new(
        dataset: SyntheticDataset,
        case_metrics: Vec<SyntheticCaseGenerationMetric>,
    ) -> DomainResult<Self> {
        if case_metrics.is_empty() {
            return Err(DomainError::EmptyCollection {
                context: "synthetic_dataset_generation_report.case_metrics".to_string(),
            });
        }

        Ok(Self {
            dataset,
            case_metrics,
        })
    }

    pub fn dataset(&self) -> &SyntheticDataset {
        &self.dataset
    }

    pub fn case_metrics(&self) -> &[SyntheticCaseGenerationMetric] {
        &self.case_metrics
    }

    pub fn total_generated(&self) -> ExampleCount {
        ExampleCount::from_usize(
            self.case_metrics
                .iter()
                .map(|metric| metric.generated().as_usize())
                .sum(),
        )
    }
}

impl SyntheticDataset {
    pub fn new(dataset_id: DatasetId, trajectories: Vec<TrainingTrajectory>) -> DomainResult<Self> {
        if trajectories.is_empty() {
            return Err(DomainError::EmptyCollection {
                context: "synthetic_dataset.trajectories".to_string(),
            });
        }

        Ok(Self {
            dataset_id,
            trajectories,
        })
    }

    pub fn dataset_id(&self) -> &DatasetId {
        &self.dataset_id
    }

    pub fn trajectories(&self) -> &[TrainingTrajectory] {
        &self.trajectories
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use underpass_operator_shared_domain::{
        ActionArguments, KernelTool, OperatorAction, OperatorMode,
    };

    #[test]
    fn synthetic_case_target_tool_must_belong_to_mode() {
        let error = SyntheticCaseSpec::new(
            SyntheticCaseId::parse("case-1").expect("case id"),
            OperatorMode::Read,
            TaskFamily::parse("synthetic.write").expect("task family"),
            KernelTool::WriteMemory,
            PositiveCount::parse(1, "minimum_examples").expect("count"),
        )
        .expect_err("write tool must not be valid in read mode");

        assert_eq!(
            error,
            DomainError::ToolOutsideMode {
                mode: "read".to_string(),
                tool: "kernel_write_memory".to_string()
            }
        );
    }

    #[test]
    fn synthetic_blueprint_must_have_cases() {
        let error =
            SyntheticDatasetBlueprint::new(DatasetId::parse("dataset-1").expect("dataset"), vec![])
                .expect_err("empty blueprint must fail");

        assert_eq!(
            error,
            DomainError::EmptyCollection {
                context: "synthetic_dataset_blueprint.cases".to_string()
            }
        );
    }

    #[test]
    fn synthetic_case_generation_metric_knows_if_minimum_was_met() {
        let metric = SyntheticCaseGenerationMetric::new(
            SyntheticCaseId::parse("case-1").expect("case"),
            PositiveCount::parse(2, "minimum_examples").expect("minimum"),
            ExampleCount::from_usize(1),
        );

        assert!(!metric.satisfies_minimum());
    }

    #[test]
    fn synthetic_case_builds_canonical_operator_trajectory() {
        let case = SyntheticCaseSpec::new(
            SyntheticCaseId::parse("case-1").expect("case"),
            OperatorMode::Read,
            TaskFamily::parse("read.inspect").expect("task family"),
            KernelTool::Inspect,
            PositiveCount::parse(1, "minimum_examples").expect("minimum"),
        )
        .expect("case");

        let trajectory = case
            .build_trajectory(OperatorTrajectoryBuildSpec::new(
                StepId::parse("step-1").expect("step"),
                AboutId::parse("about-1").expect("about"),
                VisibleState::parse(json!({ "cursor": { "ref": "node-1" } }))
                    .expect("visible state"),
                OperatorAction::tool_call(
                    KernelTool::Inspect,
                    ActionArguments::parse(json!({ "ref": "node-1" })).expect("args"),
                ),
            ))
            .expect("trajectory");

        assert_eq!(trajectory.mode(), OperatorMode::Read);
        assert_eq!(trajectory.task_family().as_str(), "read.inspect");
        assert_eq!(trajectory.allowed_tools().mode(), OperatorMode::Read);
        assert!(trajectory.allowed_tools().contains(KernelTool::Inspect));
        case.validate_trajectory(&trajectory).expect("valid case");
    }

    #[test]
    fn synthetic_case_rejects_trajectory_for_another_target_tool() {
        let case = SyntheticCaseSpec::new(
            SyntheticCaseId::parse("case-1").expect("case"),
            OperatorMode::Read,
            TaskFamily::parse("read.inspect").expect("task family"),
            KernelTool::Inspect,
            PositiveCount::parse(1, "minimum_examples").expect("minimum"),
        )
        .expect("case");

        let error = case
            .build_trajectory(OperatorTrajectoryBuildSpec::new(
                StepId::parse("step-1").expect("step"),
                AboutId::parse("about-1").expect("about"),
                VisibleState::parse(json!({})).expect("visible state"),
                OperatorAction::tool_call(
                    KernelTool::Near,
                    ActionArguments::parse(json!({ "around": { "ref": "node-1" } })).expect("args"),
                ),
            ))
            .expect_err("case target tool mismatch must fail");

        assert_eq!(
            error,
            DomainError::TrajectoryCaseMismatch {
                field: "target_action.tool".to_string(),
                expected: "kernel_inspect".to_string(),
                actual: "kernel_near".to_string()
            }
        );
    }

    #[test]
    fn synthetic_case_rejects_trajectory_that_mutilates_allowed_tools() {
        let case = SyntheticCaseSpec::new(
            SyntheticCaseId::parse("case-1").expect("case"),
            OperatorMode::Read,
            TaskFamily::parse("read.inspect").expect("task family"),
            KernelTool::Inspect,
            PositiveCount::parse(1, "minimum_examples").expect("minimum"),
        )
        .expect("case");
        let trajectory = TrainingTrajectory::new(
            StepId::parse("step-1").expect("step"),
            AboutId::parse("about-1").expect("about"),
            OperatorMode::Read,
            TaskFamily::parse("read.inspect").expect("task family"),
            AllowedTools::parse(OperatorMode::Read, vec![KernelTool::Inspect]).expect("tools"),
            VisibleState::parse(json!({})).expect("visible state"),
            OperatorAction::tool_call(
                KernelTool::Inspect,
                ActionArguments::parse(json!({ "ref": "node-1" })).expect("args"),
            ),
        )
        .expect("trajectory");

        let error = case
            .validate_trajectory(&trajectory)
            .expect_err("partial allowed tools must fail");

        assert_eq!(
            error,
            DomainError::TrajectoryCaseMismatch {
                field: "allowed_tools".to_string(),
                expected: "kernel_wake,kernel_ask,kernel_near,kernel_goto,kernel_rewind,kernel_forward,kernel_trace,kernel_inspect".to_string(),
                actual: "kernel_inspect".to_string()
            }
        );
    }

    #[test]
    fn builds_one_synthetic_case_per_kmp_mcp_capability() {
        let blueprint = SyntheticDatasetBlueprint::for_kmp_mcp_capabilities(
            DatasetId::parse("dataset-1").expect("dataset"),
            KmpMcpCapability::all(),
            PositiveCount::parse(1, "minimum_examples").expect("minimum"),
        )
        .expect("blueprint");

        assert_eq!(blueprint.cases().len(), KmpMcpCapability::all().len());
        assert_eq!(blueprint.cases()[0].target_tool(), KernelTool::Wake);
        assert_eq!(blueprint.cases()[0].mode(), OperatorMode::Read);
    }
}
