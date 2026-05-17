mod contract_coverage_observer;
mod jsonl_value_reader;
mod policy_eval_jsonl;

use underpass_operator_evaluation_application::{
    ReadEvaluationCasesError, ports::EvaluationCaseReader,
};
use underpass_operator_evaluation_domain::EvaluationCase;

pub use contract_coverage_observer::JsonContractCoverageObserver;
pub use jsonl_value_reader::{JsonlValueReadError, JsonlValueReader};
pub use policy_eval_jsonl::{
    JsonlPolicyEvalReader, PolicyEvalJsonlError, PolicyTrajectoryJsonlFormat,
};

pub struct InMemoryEvaluationCaseReader {
    cases: Vec<EvaluationCase>,
}

impl InMemoryEvaluationCaseReader {
    pub fn new(cases: Vec<EvaluationCase>) -> Self {
        Self { cases }
    }
}

impl EvaluationCaseReader for InMemoryEvaluationCaseReader {
    fn read_evaluation_cases(&self) -> Result<Vec<EvaluationCase>, ReadEvaluationCasesError> {
        Ok(self.cases.clone())
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
            "underpass-operator-evaluation-infra must stay independent from kernel crates"
        );
    }
}
