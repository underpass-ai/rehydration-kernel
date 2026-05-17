use underpass_operator_evaluation_domain::ContractEvaluationCoverageReport;

use crate::EvaluationApplicationResult;
use crate::ports::EvaluationCaseReader;

pub struct EvaluateKmpMcpContractCoverageUseCase<R> {
    reader: R,
}

impl<R> EvaluateKmpMcpContractCoverageUseCase<R>
where
    R: EvaluationCaseReader,
{
    pub fn new(reader: R) -> Self {
        Self { reader }
    }

    pub fn execute(&self) -> EvaluationApplicationResult<ContractEvaluationCoverageReport> {
        let cases = self.reader.read_evaluation_cases()?;
        Ok(ContractEvaluationCoverageReport::from_cases(&cases)?)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use underpass_operator_evaluation_domain::EvaluationCase;
    use underpass_operator_shared_domain::{
        ActionArguments, KernelTool, KmpMcpCapability, OperatorAction, StepId,
    };

    use super::*;
    use crate::ReadEvaluationCasesError;

    struct StaticReader {
        cases: Vec<EvaluationCase>,
    }

    impl EvaluationCaseReader for StaticReader {
        fn read_evaluation_cases(&self) -> Result<Vec<EvaluationCase>, ReadEvaluationCasesError> {
            Ok(self.cases.clone())
        }
    }

    #[test]
    fn evaluates_contract_capability_coverage() {
        let action = OperatorAction::tool_call(
            KernelTool::Inspect,
            ActionArguments::parse(json!({ "ref": "node-1" })).expect("arguments"),
        );
        let reader = StaticReader {
            cases: vec![EvaluationCase::new(
                StepId::parse("step-1").expect("step"),
                action.clone(),
                action,
            )],
        };

        let report = EvaluateKmpMcpContractCoverageUseCase::new(reader)
            .execute()
            .expect("report");

        assert!(
            report
                .by_capability()
                .contains_key(&KmpMcpCapability::from_tool(KernelTool::Inspect))
        );
    }
}
