use underpass_operator_shared_domain::{DatasetId, KmpMcpCapability, PositiveCount};
use underpass_operator_synthetic_domain::SyntheticDatasetBlueprint;

use crate::SyntheticApplicationResult;

pub struct PlanKmpMcpSyntheticCasesUseCase;

impl PlanKmpMcpSyntheticCasesUseCase {
    pub fn execute(
        dataset_id: DatasetId,
        minimum_examples_per_capability: PositiveCount,
    ) -> SyntheticApplicationResult<SyntheticDatasetBlueprint> {
        Ok(SyntheticDatasetBlueprint::for_kmp_mcp_capabilities(
            dataset_id,
            KmpMcpCapability::all(),
            minimum_examples_per_capability,
        )?)
    }
}

#[cfg(test)]
mod tests {
    use underpass_operator_shared_domain::{DatasetId, KmpMcpCapability, PositiveCount};

    use super::*;

    #[test]
    fn creates_a_synthetic_case_for_every_contract_capability() {
        let blueprint = PlanKmpMcpSyntheticCasesUseCase::execute(
            DatasetId::parse("dataset-1").expect("dataset"),
            PositiveCount::parse(2, "minimum_examples").expect("minimum"),
        )
        .expect("blueprint");

        assert_eq!(blueprint.cases().len(), KmpMcpCapability::all().len());
        assert!(
            blueprint
                .cases()
                .iter()
                .all(|case| case.minimum_examples().as_usize() == 2)
        );
    }
}
