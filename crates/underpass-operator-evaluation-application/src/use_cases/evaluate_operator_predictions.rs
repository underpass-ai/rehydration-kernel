use underpass_operator_evaluation_domain::EvaluationReport;

use crate::EvaluationApplicationResult;
use crate::ports::EvaluationCaseReader;

pub struct EvaluateOperatorPredictionsUseCase<R> {
    reader: R,
}

impl<R> EvaluateOperatorPredictionsUseCase<R>
where
    R: EvaluationCaseReader,
{
    pub fn new(reader: R) -> Self {
        Self { reader }
    }

    pub fn execute(&self) -> EvaluationApplicationResult<EvaluationReport> {
        let cases = self.reader.read_evaluation_cases()?;
        Ok(EvaluationReport::from_cases(cases)?)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use underpass_operator_evaluation_domain::EvaluationCase;
    use underpass_operator_shared_domain::{ActionArguments, KernelTool, OperatorAction, StepId};

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
    fn evaluates_prediction_cases_into_domain_report() {
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

        let report = EvaluateOperatorPredictionsUseCase::new(reader)
            .execute()
            .expect("report");

        assert_eq!(report.accuracy_basis_points(), 10_000);
    }
}
