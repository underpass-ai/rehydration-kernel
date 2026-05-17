use std::collections::{BTreeMap, BTreeSet};

use underpass_operator_shared_domain::{
    DomainError, DomainResult, ExampleCount, KmpMcpCapability, OperatorAction, StepId,
    operator_action_contract_error, operator_allowed_full_tools, operator_allowed_read_tools,
    operator_allowed_write_tools, operator_allowed_writer_pre_read_tools,
    operator_is_bounded_tool_call,
};

mod policy_eval;

pub use policy_eval::{
    POLICY_ACTION_VALIDATOR, POLICY_EVALUATOR, POLICY_SCHEMA_MODE, PolicyActionScore,
    PolicyEvalBaseline, PolicyEvalBreakdown, PolicyEvalCounts, PolicyEvalDetail, PolicyEvalRates,
    PolicyEvalRequest, PolicyEvalResult, PolicyEvalSummary, PolicyEvalTrajectory, PolicyEvaluator,
    policy_action_type, policy_deterministic_action, policy_tool,
};

#[derive(Debug, Clone, PartialEq)]
pub struct EvaluationCase {
    step_id: StepId,
    expected_action: OperatorAction,
    predicted_action: OperatorAction,
}

impl EvaluationCase {
    pub fn new(
        step_id: StepId,
        expected_action: OperatorAction,
        predicted_action: OperatorAction,
    ) -> Self {
        Self {
            step_id,
            expected_action,
            predicted_action,
        }
    }

    pub fn step_id(&self) -> &StepId {
        &self.step_id
    }

    pub fn expected_action(&self) -> &OperatorAction {
        &self.expected_action
    }

    pub fn predicted_action(&self) -> &OperatorAction {
        &self.predicted_action
    }

    pub fn evaluate(&self) -> EvaluationOutcome {
        let verdict = if self.expected_action == self.predicted_action {
            EvaluationVerdict::ExactMatch
        } else {
            EvaluationVerdict::Mismatch
        };

        EvaluationOutcome {
            step_id: self.step_id.clone(),
            verdict,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvaluationVerdict {
    ExactMatch,
    Mismatch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluationOutcome {
    step_id: StepId,
    verdict: EvaluationVerdict,
}

impl EvaluationOutcome {
    pub fn step_id(&self) -> &StepId {
        &self.step_id
    }

    pub fn verdict(&self) -> EvaluationVerdict {
        self.verdict
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluationReport {
    outcomes: Vec<EvaluationOutcome>,
}

impl EvaluationReport {
    pub fn from_cases(cases: Vec<EvaluationCase>) -> DomainResult<Self> {
        if cases.is_empty() {
            return Err(DomainError::EmptyCollection {
                context: "evaluation_cases".to_string(),
            });
        }

        let outcomes = cases.into_iter().map(|case| case.evaluate()).collect();
        Self::new(outcomes)
    }

    pub fn new(outcomes: Vec<EvaluationOutcome>) -> DomainResult<Self> {
        if outcomes.is_empty() {
            return Err(DomainError::EmptyCollection {
                context: "evaluation_report.outcomes".to_string(),
            });
        }

        Ok(Self { outcomes })
    }

    pub fn outcomes(&self) -> &[EvaluationOutcome] {
        &self.outcomes
    }

    pub fn total(&self) -> ExampleCount {
        ExampleCount::from_usize(self.outcomes.len())
    }

    pub fn exact_matches(&self) -> ExampleCount {
        ExampleCount::from_usize(
            self.outcomes
                .iter()
                .filter(|outcome| outcome.verdict == EvaluationVerdict::ExactMatch)
                .count(),
        )
    }

    pub fn accuracy_basis_points(&self) -> u16 {
        self.metrics().accuracy_basis_points()
    }

    pub fn metrics(&self) -> EvaluationMetrics {
        EvaluationMetrics::from_outcomes(&self.outcomes)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractEvaluationCoverageReport {
    by_capability: BTreeMap<KmpMcpCapability, EvaluationMetrics>,
    missing_capabilities: Vec<KmpMcpCapability>,
}

impl ContractEvaluationCoverageReport {
    pub fn from_cases(cases: &[EvaluationCase]) -> DomainResult<Self> {
        if cases.is_empty() {
            return Err(DomainError::EmptyCollection {
                context: "evaluation_cases".to_string(),
            });
        }

        let mut by_capability = BTreeMap::new();
        let mut missing_capabilities = Vec::new();

        for capability in KmpMcpCapability::all().iter().copied() {
            let outcomes = cases
                .iter()
                .filter(|case| {
                    KmpMcpCapability::from_action(case.expected_action())
                        .is_some_and(|expected| expected == capability)
                })
                .map(EvaluationCase::evaluate)
                .collect::<Vec<_>>();

            if outcomes.is_empty() {
                missing_capabilities.push(capability);
            } else {
                by_capability.insert(capability, EvaluationReport::new(outcomes)?.metrics());
            }
        }

        Ok(Self {
            by_capability,
            missing_capabilities,
        })
    }

    pub fn by_capability(&self) -> &BTreeMap<KmpMcpCapability, EvaluationMetrics> {
        &self.by_capability
    }

    pub fn missing_capabilities(&self) -> &[KmpMcpCapability] {
        &self.missing_capabilities
    }

    pub fn covered_capabilities(&self) -> ExampleCount {
        ExampleCount::from_usize(self.by_capability.len())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvaluationMetrics {
    total: ExampleCount,
    exact_matches: ExampleCount,
    mismatches: ExampleCount,
    accuracy_basis_points: u16,
}

impl EvaluationMetrics {
    fn from_outcomes(outcomes: &[EvaluationOutcome]) -> Self {
        let total = outcomes.len();
        let exact = outcomes
            .iter()
            .filter(|outcome| outcome.verdict == EvaluationVerdict::ExactMatch)
            .count();
        let mismatches = total - exact;
        let accuracy_basis_points = ((exact * 10_000) / total) as u16;

        Self {
            total: ExampleCount::from_usize(total),
            exact_matches: ExampleCount::from_usize(exact),
            mismatches: ExampleCount::from_usize(mismatches),
            accuracy_basis_points,
        }
    }

    pub fn total(self) -> ExampleCount {
        self.total
    }

    pub fn exact_matches(self) -> ExampleCount {
        self.exact_matches
    }

    pub fn mismatches(self) -> ExampleCount {
        self.mismatches
    }

    pub fn accuracy_basis_points(self) -> u16 {
        self.accuracy_basis_points
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractCoverageProfile {
    Read,
    Full,
    WriterPreRead,
    Write,
}

impl ContractCoverageProfile {
    pub fn parse(value: &str) -> DomainResult<Self> {
        match value {
            "read" => Ok(Self::Read),
            "full" => Ok(Self::Full),
            "writer-pre-read" => Ok(Self::WriterPreRead),
            "write" => Ok(Self::Write),
            other => Err(DomainError::UnsupportedMode {
                value: other.to_string(),
            }),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Full => "full",
            Self::WriterPreRead => "writer-pre-read",
            Self::Write => "write",
        }
    }

    pub fn operator_tool_names(self) -> Vec<String> {
        match self {
            Self::Read => operator_allowed_read_tools(),
            Self::Full => operator_allowed_full_tools(),
            Self::WriterPreRead => operator_allowed_writer_pre_read_tools(),
            Self::Write => operator_allowed_write_tools(),
        }
    }

    pub fn required_capabilities(self) -> Vec<ContractCapability> {
        match self {
            Self::Read => read_required_capabilities(),
            Self::Full => {
                let mut capabilities = read_required_capabilities();
                capabilities.extend(write_extra_capabilities());
                capabilities
            }
            Self::WriterPreRead => writer_pre_read_required_capabilities(),
            Self::Write => write_required_capabilities(),
        }
    }

    pub fn includes_mode(self, mode: &str) -> bool {
        match self {
            Self::Full => true,
            Self::Read => !matches!(mode, "write_context_read" | "write"),
            Self::WriterPreRead => mode == "write_context_read",
            Self::Write => mode == "write",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContractCapability {
    id: &'static str,
    group: &'static str,
}

impl ContractCapability {
    const fn new(id: &'static str, group: &'static str) -> Self {
        Self { id, group }
    }

    pub fn id(self) -> &'static str {
        self.id
    }

    pub fn group(self) -> &'static str {
        self.group
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContractProfileCoverageReport {
    overall_contract_coverage: ContractCoverageRatio,
    profile_contract_coverage: ContractCoverageRatio,
    required_capabilities: Vec<ContractCapabilityCoverageRow>,
    unsupported_mcp_tools: Vec<String>,
}

impl ContractProfileCoverageReport {
    pub fn evaluate(
        profile: ContractCoverageProfile,
        mcp_tools: &BTreeSet<String>,
        observed_capabilities: Option<&BTreeSet<String>>,
    ) -> Self {
        let operator_tools = profile
            .operator_tool_names()
            .into_iter()
            .collect::<BTreeSet<_>>();
        let required_capabilities = profile
            .required_capabilities()
            .into_iter()
            .map(|capability| {
                ContractCapabilityCoverageRow::new(
                    capability,
                    contract_supports(capability.id(), mcp_tools, &operator_tools),
                    observed_capabilities
                        .as_ref()
                        .map(|observed| observed.contains(capability.id())),
                )
            })
            .collect::<Vec<_>>();
        let profile_supported = required_capabilities
            .iter()
            .filter(|row| row.contract_supported())
            .count();
        let unsupported_mcp_tools = mcp_tools
            .iter()
            .filter(|tool| !operator_tools.contains(*tool))
            .cloned()
            .collect::<Vec<_>>();

        Self {
            overall_contract_coverage: ContractCoverageRatio::new(
                mcp_tools
                    .iter()
                    .filter(|tool| operator_tools.contains(*tool))
                    .count(),
                mcp_tools.len(),
            ),
            profile_contract_coverage: ContractCoverageRatio::new(
                profile_supported,
                required_capabilities.len(),
            ),
            required_capabilities,
            unsupported_mcp_tools,
        }
    }

    pub fn overall_contract_coverage(&self) -> ContractCoverageRatio {
        self.overall_contract_coverage
    }

    pub fn profile_contract_coverage(&self) -> ContractCoverageRatio {
        self.profile_contract_coverage
    }

    pub fn required_capabilities(&self) -> &[ContractCapabilityCoverageRow] {
        &self.required_capabilities
    }

    pub fn unsupported_mcp_tools(&self) -> &[String] {
        &self.unsupported_mcp_tools
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContractCapabilityCoverageRow {
    capability: ContractCapability,
    contract_supported: bool,
    training_observed: Option<bool>,
}

impl ContractCapabilityCoverageRow {
    const fn new(
        capability: ContractCapability,
        contract_supported: bool,
        training_observed: Option<bool>,
    ) -> Self {
        Self {
            capability,
            contract_supported,
            training_observed,
        }
    }

    pub fn capability(self) -> ContractCapability {
        self.capability
    }

    pub fn contract_supported(self) -> bool {
        self.contract_supported
    }

    pub fn training_observed(self) -> Option<bool> {
        self.training_observed
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContractCoverageRatio {
    covered: usize,
    total: usize,
    percent: f64,
}

impl ContractCoverageRatio {
    pub fn new(covered: usize, total: usize) -> Self {
        let percent = if total == 0 {
            100.0
        } else {
            covered as f64 * 100.0 / total as f64
        };
        Self {
            covered,
            total,
            percent,
        }
    }

    pub fn covered(self) -> usize {
        self.covered
    }

    pub fn total(self) -> usize {
        self.total
    }

    pub fn percent(self) -> f64 {
        self.percent
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContractTrainingCoverageObservation {
    pub rows_total: usize,
    pub rows_included: usize,
    pub rows_skipped_by_profile: usize,
    pub row_parse_failures: usize,
    pub row_parse_failure_examples: Vec<String>,
    pub capabilities: BTreeSet<&'static str>,
    pub target_tools: BTreeMap<String, usize>,
    pub cursor_modes: BTreeMap<String, usize>,
    pub dimension_modes: BTreeMap<String, usize>,
    pub dimension_scopes: BTreeMap<String, usize>,
    pub dimension_scope_ids: BTreeMap<String, usize>,
    pub trace_page_modes: BTreeMap<String, usize>,
    pub answer_policies: BTreeMap<String, usize>,
    pub budget_details: BTreeMap<String, usize>,
    pub temporal_raw_refs: BTreeMap<String, usize>,
    pub inspect_raw: BTreeMap<String, usize>,
    pub write_memory_options: BTreeMap<String, usize>,
    pub write_memory_dry_run: BTreeMap<String, usize>,
    pub write_memory_strict: BTreeMap<String, usize>,
    pub write_memory_idempotency_key: BTreeMap<String, usize>,
    pub write_memory_read_context: BTreeMap<String, usize>,
    pub write_memory_current_evidence: BTreeMap<String, usize>,
    pub write_memory_source_kind: BTreeMap<String, usize>,
    pub write_memory_relation_proof: BTreeMap<String, usize>,
    pub ingest_dry_run: BTreeMap<String, usize>,
    pub ingest_dimensions: BTreeMap<String, usize>,
    pub ingest_relations: BTreeMap<String, usize>,
    pub ingest_evidence: BTreeMap<String, usize>,
    pub ingest_provenance: BTreeMap<String, usize>,
    pub action_contract_failures: usize,
    pub action_contract_failure_phases: BTreeMap<String, usize>,
    pub action_contract_failure_examples: Vec<String>,
}

impl ContractTrainingCoverageObservation {
    pub fn capability_ids(&self) -> BTreeSet<String> {
        self.capabilities
            .iter()
            .map(|capability| (*capability).to_string())
            .collect()
    }
}

fn read_required_capabilities() -> Vec<ContractCapability> {
    vec![
        ContractCapability::new("tool:kernel_wake", "tool"),
        ContractCapability::new("tool:kernel_ask", "tool"),
        ContractCapability::new("tool:kernel_near", "tool"),
        ContractCapability::new("tool:kernel_goto", "tool"),
        ContractCapability::new("tool:kernel_rewind", "tool"),
        ContractCapability::new("tool:kernel_forward", "tool"),
        ContractCapability::new("tool:kernel_trace", "tool"),
        ContractCapability::new("tool:kernel_inspect", "tool"),
        ContractCapability::new("tool:stop", "tool"),
        ContractCapability::new("cursor:ref", "cursor"),
        ContractCapability::new("cursor:time", "cursor"),
        ContractCapability::new("cursor:sequence", "cursor"),
        ContractCapability::new("dimensions.mode:all", "dimensions"),
        ContractCapability::new("dimensions.mode:only", "dimensions"),
        ContractCapability::new("dimensions.mode:except", "dimensions"),
        ContractCapability::new("dimensions.scope:current_about", "dimensions"),
        ContractCapability::new("dimensions.scope:abouts", "dimensions"),
        ContractCapability::new("dimensions.scope:all_abouts", "dimensions"),
        ContractCapability::new("trace.page:first", "pagination"),
        ContractCapability::new("trace.page:continue", "pagination"),
        ContractCapability::new("window:expand", "window_policy"),
        ContractCapability::new("window:shrink", "window_policy"),
        ContractCapability::new("window:stop_sufficient", "window_policy"),
        ContractCapability::new("inspect.raw:false", "security"),
    ]
}

fn writer_pre_read_required_capabilities() -> Vec<ContractCapability> {
    vec![
        ContractCapability::new("mode:write_context_read", "mode"),
        ContractCapability::new("tool:kernel_near", "tool"),
        ContractCapability::new("tool:kernel_inspect", "tool"),
        ContractCapability::new("tool:kernel_trace", "tool"),
        ContractCapability::new("tool:stop", "tool"),
        ContractCapability::new("cursor:ref", "cursor"),
        ContractCapability::new("dimensions.mode:all", "dimensions"),
        ContractCapability::new("dimensions.scope:current_about", "dimensions"),
        ContractCapability::new("window:expand", "window_policy"),
        ContractCapability::new("window:shrink", "window_policy"),
        ContractCapability::new("inspect.raw:false", "security"),
        ContractCapability::new("trace.page:first", "pagination"),
        ContractCapability::new("trace.page:continue", "pagination"),
        ContractCapability::new("window:stop_sufficient", "window_policy"),
        ContractCapability::new("writer.last_tool:none", "writer_state"),
        ContractCapability::new("writer.last_tool:kernel_near", "writer_state"),
        ContractCapability::new("writer.last_tool:kernel_inspect", "writer_state"),
        ContractCapability::new("writer.last_tool:kernel_trace", "writer_state"),
        ContractCapability::new(
            "writer.candidate_role:previous_subtask_answer",
            "writer_candidate",
        ),
        ContractCapability::new(
            "writer.candidate_role:same_subtask_question",
            "writer_candidate",
        ),
        ContractCapability::new("writer.candidate_pool:ambiguous", "writer_candidate"),
    ]
}

fn write_extra_capabilities() -> Vec<ContractCapability> {
    vec![
        ContractCapability::new("tool:kernel_ingest", "tool"),
        ContractCapability::new("tool:kernel_write_memory", "tool"),
        ContractCapability::new("write:relation_quality", "write"),
        ContractCapability::new("write:read_context_proof", "write"),
    ]
}

fn write_required_capabilities() -> Vec<ContractCapability> {
    vec![
        ContractCapability::new("mode:write", "mode"),
        ContractCapability::new("tool:kernel_ingest", "tool"),
        ContractCapability::new("tool:kernel_write_memory", "tool"),
        ContractCapability::new("tool:stop", "tool"),
        ContractCapability::new(
            "prepared.source:draft_write.prepared_arguments",
            "prepared_payload",
        ),
        ContractCapability::new("prepared.source:canonical_payload", "prepared_payload"),
        ContractCapability::new("write:relation_quality", "write"),
        ContractCapability::new("write:read_context_proof", "write"),
        ContractCapability::new("window:stop_sufficient", "window_policy"),
    ]
}

fn contract_supports(
    id: &str,
    mcp_tools: &BTreeSet<String>,
    operator_tools: &BTreeSet<String>,
) -> bool {
    if let Some(tool) = id.strip_prefix("tool:") {
        return tool == "stop" || (mcp_tools.contains(tool) && operator_tools.contains(tool));
    }
    match id {
        "cursor:ref" | "cursor:time" | "cursor:sequence" => bounded_temporal_cursor(id),
        "dimensions.mode:all" | "dimensions.mode:only" | "dimensions.mode:except" => {
            validated_dimension_mode(id)
        }
        "dimensions.scope:current_about"
        | "dimensions.scope:abouts"
        | "dimensions.scope:all_abouts" => validated_dimension_scope(id),
        "trace.page:first" | "trace.page:continue" => bounded_trace_page(id),
        "window:expand" | "window:shrink" | "window:stop_sufficient" => true,
        "inspect.raw:false" => bounded_inspect_raw_false(),
        "mode:write"
        | "prepared.source:draft_write.prepared_arguments"
        | "prepared.source:canonical_payload" => true,
        "mode:write_context_read"
        | "writer.last_tool:none"
        | "writer.last_tool:kernel_near"
        | "writer.last_tool:kernel_inspect"
        | "writer.last_tool:kernel_trace"
        | "writer.candidate_role:previous_subtask_answer"
        | "writer.candidate_role:same_subtask_question"
        | "writer.candidate_pool:ambiguous" => true,
        "write:relation_quality" | "write:read_context_proof" => {
            mcp_tools.contains("kernel_write_memory")
                && operator_tools.contains("kernel_write_memory")
        }
        _ => false,
    }
}

fn bounded_temporal_cursor(id: &str) -> bool {
    let cursor = match id {
        "cursor:ref" => serde_json::json!({ "ref": "node:1" }),
        "cursor:time" => serde_json::json!({ "time": "2026-05-14T00:00:00Z" }),
        "cursor:sequence" => serde_json::json!({ "sequence": 1 }),
        _ => return false,
    };
    let arguments = serde_json::json!({
        "about": "about:1",
        "around": cursor,
        "dimensions": { "mode": "all", "scope": "current_about" },
        "include": { "evidence": true, "raw_refs": false, "relations": true },
        "limit": { "entries": 12, "tokens": 2400 },
        "budget": { "depth": 3, "tokens": 2400 },
        "window": { "before_entries": 6, "after_entries": 0 }
    });
    operator_is_bounded_tool_call("kernel_near", &arguments)
}

fn validated_dimension_mode(id: &str) -> bool {
    let dimensions = match id {
        "dimensions.mode:all" => serde_json::json!({ "mode": "all", "scope": "current_about" }),
        "dimensions.mode:only" => {
            serde_json::json!({ "mode": "only", "include": ["agent"], "scope": "current_about" })
        }
        "dimensions.mode:except" => {
            serde_json::json!({ "mode": "except", "exclude": ["discarded"], "scope": "current_about" })
        }
        _ => return false,
    };
    action_contract_accepts_dimensions(dimensions)
}

fn validated_dimension_scope(id: &str) -> bool {
    let dimensions = match id {
        "dimensions.scope:current_about" => {
            serde_json::json!({ "mode": "all", "scope": "current_about" })
        }
        "dimensions.scope:abouts" => {
            serde_json::json!({ "mode": "all", "scope": "abouts", "abouts": ["about:1"] })
        }
        "dimensions.scope:all_abouts" => {
            serde_json::json!({ "mode": "all", "scope": "all_abouts" })
        }
        _ => return false,
    };
    action_contract_accepts_dimensions(dimensions)
}

fn action_contract_accepts_dimensions(dimensions: serde_json::Value) -> bool {
    let action = serde_json::json!({
        "type": "tool_call",
        "tool": "kernel_ask",
        "arguments": {
            "about": "about:1",
            "answer_policy": "evidence_or_unknown",
            "dimensions": dimensions,
            "question": "What changed?",
            "budget": { "tokens": 2400 }
        }
    });
    operator_action_contract_error(&action).is_none()
}

fn bounded_trace_page(id: &str) -> bool {
    let page = match id {
        "trace.page:first" => serde_json::json!({ "entries": 16 }),
        "trace.page:continue" => serde_json::json!({ "entries": 16, "cursor": "16" }),
        _ => return false,
    };
    let arguments = serde_json::json!({
        "from": "node:1",
        "to": "node:2",
        "budget": { "depth": 2, "tokens": 2400 },
        "page": page
    });
    operator_is_bounded_tool_call("kernel_trace", &arguments)
}

fn bounded_inspect_raw_false() -> bool {
    operator_is_bounded_tool_call(
        "kernel_inspect",
        &serde_json::json!({
            "ref": "node:1",
            "include": { "details": true, "incoming": true, "outgoing": true, "raw": false }
        }),
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use serde_json::json;
    use underpass_operator_shared_domain::{ActionArguments, KernelTool};

    use super::*;

    #[test]
    fn exact_action_match_scores_as_match() {
        let action = OperatorAction::tool_call(
            KernelTool::Inspect,
            ActionArguments::parse(json!({ "ref": "node-1" })).expect("arguments"),
        );
        let case = EvaluationCase::new(
            StepId::parse("step-1").expect("step"),
            action.clone(),
            action,
        );

        let report = EvaluationReport::new(vec![case.evaluate()]).expect("report");

        assert_eq!(report.total().as_usize(), 1);
        assert_eq!(report.exact_matches().as_usize(), 1);
        assert_eq!(report.accuracy_basis_points(), 10_000);
        assert_eq!(report.metrics().mismatches().as_usize(), 0);
    }

    #[test]
    fn empty_evaluation_report_fails_fast() {
        let error = EvaluationReport::new(vec![]).expect_err("empty report must fail");

        assert_eq!(
            error,
            DomainError::EmptyCollection {
                context: "evaluation_report.outcomes".to_string()
            }
        );
    }

    #[test]
    fn report_from_cases_owns_case_scoring() {
        let action = OperatorAction::tool_call(
            KernelTool::Inspect,
            ActionArguments::parse(json!({ "ref": "node-1" })).expect("arguments"),
        );
        let report = EvaluationReport::from_cases(vec![EvaluationCase::new(
            StepId::parse("step-1").expect("step"),
            action.clone(),
            action,
        )])
        .expect("report");

        assert_eq!(report.metrics().exact_matches().as_usize(), 1);
    }

    #[test]
    fn contract_coverage_report_is_built_from_expected_actions() {
        let action = OperatorAction::tool_call(
            KernelTool::Inspect,
            ActionArguments::parse(json!({ "ref": "node-1" })).expect("arguments"),
        );
        let report = ContractEvaluationCoverageReport::from_cases(&[EvaluationCase::new(
            StepId::parse("step-1").expect("step"),
            action.clone(),
            action,
        )])
        .expect("coverage report");

        assert!(
            report
                .by_capability()
                .contains_key(&KmpMcpCapability::from_tool(KernelTool::Inspect))
        );
        assert_eq!(
            report.missing_capabilities().len(),
            KmpMcpCapability::all().len() - 1
        );
    }

    #[test]
    fn profile_required_capabilities_are_domain_owned() {
        let write_ids = ContractCoverageProfile::Write
            .required_capabilities()
            .into_iter()
            .map(ContractCapability::id)
            .collect::<Vec<_>>();

        assert!(write_ids.contains(&"tool:kernel_ingest"));
        assert!(write_ids.contains(&"tool:kernel_write_memory"));
        assert!(write_ids.contains(&"prepared.source:canonical_payload"));
    }

    #[test]
    fn profile_contract_coverage_uses_shared_action_contract() {
        let mcp_tools = [
            "kernel_wake",
            "kernel_ask",
            "kernel_near",
            "kernel_goto",
            "kernel_rewind",
            "kernel_forward",
            "kernel_trace",
            "kernel_inspect",
            "kernel_ingest",
            "kernel_write_memory",
        ]
        .into_iter()
        .map(ToString::to_string)
        .collect::<BTreeSet<_>>();
        let observed = ["tool:kernel_near", "cursor:ref"]
            .into_iter()
            .map(ToString::to_string)
            .collect::<BTreeSet<_>>();

        let report = ContractProfileCoverageReport::evaluate(
            ContractCoverageProfile::Read,
            &mcp_tools,
            Some(&observed),
        );

        assert_eq!(report.profile_contract_coverage().percent(), 100.0);
        assert!(
            report
                .required_capabilities()
                .iter()
                .any(|row| row.capability().id() == "cursor:ref"
                    && row.training_observed() == Some(true))
        );
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
            "underpass-operator-evaluation-domain must stay independent from kernel crates"
        );
    }
}
