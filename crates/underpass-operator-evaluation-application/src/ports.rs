use underpass_operator_evaluation_domain::EvaluationCase;

use crate::ReadEvaluationCasesError;

pub trait EvaluationCaseReader {
    fn read_evaluation_cases(&self) -> Result<Vec<EvaluationCase>, ReadEvaluationCasesError>;
}
