use std::collections::BTreeMap;

use underpass_operator_shared_domain::{
    ArtifactUri, DatasetId, DomainError, DomainResult, ExampleCount, KmpMcpCapability, ModelId,
    PositiveCount, TrainingRunId, TrainingTrajectory,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrainingDatasetManifest {
    dataset_id: DatasetId,
    artifact_uri: ArtifactUri,
    examples: PositiveCount,
}

impl TrainingDatasetManifest {
    pub fn new(dataset_id: DatasetId, artifact_uri: ArtifactUri, examples: PositiveCount) -> Self {
        Self {
            dataset_id,
            artifact_uri,
            examples,
        }
    }

    pub fn dataset_id(&self) -> &DatasetId {
        &self.dataset_id
    }

    pub fn artifact_uri(&self) -> &ArtifactUri {
        &self.artifact_uri
    }

    pub fn examples(&self) -> PositiveCount {
        self.examples
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrainingRunPlan {
    run_id: TrainingRunId,
    dataset: TrainingDatasetManifest,
    base_model: ModelId,
    epochs: PositiveCount,
    eval_examples: PositiveCount,
}

impl TrainingRunPlan {
    pub fn new(
        run_id: TrainingRunId,
        dataset: TrainingDatasetManifest,
        base_model: ModelId,
        epochs: PositiveCount,
        eval_examples: PositiveCount,
    ) -> DomainResult<Self> {
        if eval_examples.as_usize() > dataset.examples().as_usize() {
            return Err(DomainError::CountExceeds {
                context: "training_run.eval_examples".to_string(),
                max: dataset.examples().as_usize(),
                actual: eval_examples.as_usize(),
            });
        }

        Ok(Self {
            run_id,
            dataset,
            base_model,
            epochs,
            eval_examples,
        })
    }

    pub fn run_id(&self) -> &TrainingRunId {
        &self.run_id
    }

    pub fn dataset(&self) -> &TrainingDatasetManifest {
        &self.dataset
    }

    pub fn base_model(&self) -> &ModelId {
        &self.base_model
    }

    pub fn epochs(&self) -> PositiveCount {
        self.epochs
    }

    pub fn eval_examples(&self) -> PositiveCount {
        self.eval_examples
    }

    pub fn metrics(&self) -> TrainingRunMetrics {
        TrainingRunMetrics::from_plan(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrainingRunMetrics {
    dataset_examples: PositiveCount,
    eval_examples: PositiveCount,
    train_examples: ExampleCount,
    epochs: PositiveCount,
    eval_ratio_basis_points: u16,
}

impl TrainingRunMetrics {
    fn from_plan(plan: &TrainingRunPlan) -> Self {
        let dataset_examples = plan.dataset().examples();
        let eval_examples = plan.eval_examples();
        let train_examples = dataset_examples
            .as_usize()
            .saturating_sub(eval_examples.as_usize());
        let eval_ratio_basis_points =
            ((eval_examples.as_usize() * 10_000) / dataset_examples.as_usize()) as u16;

        Self {
            dataset_examples,
            eval_examples,
            train_examples: ExampleCount::from_usize(train_examples),
            epochs: plan.epochs(),
            eval_ratio_basis_points,
        }
    }

    pub fn dataset_examples(self) -> PositiveCount {
        self.dataset_examples
    }

    pub fn eval_examples(self) -> PositiveCount {
        self.eval_examples
    }

    pub fn train_examples(self) -> ExampleCount {
        self.train_examples
    }

    pub fn epochs(self) -> PositiveCount {
        self.epochs
    }

    pub fn eval_ratio_basis_points(self) -> u16 {
        self.eval_ratio_basis_points
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrainingRunReadinessReport {
    plan: TrainingRunPlan,
    metrics: TrainingRunMetrics,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrainingContractCoverageReport {
    by_capability: BTreeMap<KmpMcpCapability, ExampleCount>,
    missing_capabilities: Vec<KmpMcpCapability>,
}

impl TrainingContractCoverageReport {
    pub fn from_trajectories(trajectories: &[TrainingTrajectory]) -> DomainResult<Self> {
        if trajectories.is_empty() {
            return Err(DomainError::EmptyCollection {
                context: "training_contract_coverage.trajectories".to_string(),
            });
        }

        let mut raw_counts = BTreeMap::<KmpMcpCapability, usize>::new();
        for trajectory in trajectories {
            if let Some(capability) = KmpMcpCapability::from_action(trajectory.target_action()) {
                *raw_counts.entry(capability).or_default() += 1;
            }
        }

        let by_capability = raw_counts
            .into_iter()
            .map(|(capability, count)| (capability, ExampleCount::from_usize(count)))
            .collect::<BTreeMap<_, _>>();
        let missing_capabilities = KmpMcpCapability::all()
            .iter()
            .copied()
            .filter(|capability| !by_capability.contains_key(capability))
            .collect();

        Ok(Self {
            by_capability,
            missing_capabilities,
        })
    }

    pub fn by_capability(&self) -> &BTreeMap<KmpMcpCapability, ExampleCount> {
        &self.by_capability
    }

    pub fn missing_capabilities(&self) -> &[KmpMcpCapability] {
        &self.missing_capabilities
    }

    pub fn covered_capabilities(&self) -> ExampleCount {
        ExampleCount::from_usize(self.by_capability.len())
    }
}

impl TrainingRunReadinessReport {
    pub fn new(plan: TrainingRunPlan) -> Self {
        let metrics = plan.metrics();
        Self { plan, metrics }
    }

    pub fn plan(&self) -> &TrainingRunPlan {
        &self.plan
    }

    pub fn metrics(&self) -> TrainingRunMetrics {
        self.metrics
    }
}

#[cfg(test)]
mod tests {
    use underpass_operator_shared_domain::{
        ActionArguments, AllowedTools, ArtifactUri, DatasetId, KernelTool, ModelId, OperatorAction,
        OperatorMode, PositiveCount, StepId, TaskFamily, TrainingRunId, VisibleState,
    };

    use super::*;

    #[test]
    fn training_run_metrics_are_domain_values() {
        let plan = training_run_plan();
        let report = TrainingRunReadinessReport::new(plan);
        let metrics = report.metrics();

        assert_eq!(metrics.dataset_examples().as_usize(), 200);
        assert_eq!(metrics.train_examples().as_usize(), 160);
        assert_eq!(metrics.eval_ratio_basis_points(), 2_000);
    }

    #[test]
    fn training_contract_coverage_counts_target_actions_by_capability() {
        let report =
            TrainingContractCoverageReport::from_trajectories(&[trajectory()]).expect("coverage");

        assert_eq!(
            report
                .by_capability()
                .get(&KmpMcpCapability::from_tool(KernelTool::Inspect))
                .copied(),
            Some(ExampleCount::from_usize(1))
        );
        assert_eq!(
            report.missing_capabilities().len(),
            KmpMcpCapability::all().len() - 1
        );
    }

    fn training_run_plan() -> TrainingRunPlan {
        let manifest = TrainingDatasetManifest::new(
            DatasetId::parse("dataset-1").expect("dataset"),
            ArtifactUri::parse("file://operator/dataset.jsonl").expect("artifact"),
            PositiveCount::parse(200, "examples").expect("examples"),
        );

        TrainingRunPlan::new(
            TrainingRunId::parse("run-1").expect("run"),
            manifest,
            ModelId::parse("operator-0.5b").expect("model"),
            PositiveCount::parse(3, "epochs").expect("epochs"),
            PositiveCount::parse(40, "eval_examples").expect("eval examples"),
        )
        .expect("plan")
    }

    fn trajectory() -> TrainingTrajectory {
        let mode = OperatorMode::Read;
        TrainingTrajectory::new(
            StepId::parse("step-1").expect("step"),
            underpass_operator_shared_domain::AboutId::parse("about-1").expect("about"),
            mode,
            TaskFamily::parse("read.inspect").expect("task"),
            AllowedTools::parse(mode, vec![KernelTool::Inspect]).expect("tools"),
            VisibleState::parse(serde_json::json!({})).expect("visible"),
            OperatorAction::tool_call(
                KernelTool::Inspect,
                ActionArguments::parse(serde_json::json!({ "ref": "node-1" })).expect("arguments"),
            ),
        )
        .expect("trajectory")
    }
}

#[cfg(test)]
mod dependency_tests {
    use std::fs;
    use std::path::Path;

    #[test]
    fn crate_has_no_rehydration_dependencies() {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        let contents = fs::read_to_string(manifest).expect("manifest must be readable");

        assert!(
            !contents.contains("rehydration-"),
            "underpass-operator-training-domain must stay independent from kernel crates"
        );
    }
}
