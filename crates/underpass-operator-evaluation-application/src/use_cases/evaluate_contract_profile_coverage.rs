use std::collections::BTreeSet;

use underpass_operator_evaluation_domain::{
    ContractCoverageProfile, ContractProfileCoverageReport,
};

pub struct EvaluateContractProfileCoverageUseCase;

impl EvaluateContractProfileCoverageUseCase {
    pub fn new() -> Self {
        Self
    }

    pub fn execute(
        &self,
        profile: ContractCoverageProfile,
        mcp_tools: BTreeSet<String>,
        observed_capabilities: Option<BTreeSet<String>>,
    ) -> ContractProfileCoverageReport {
        ContractProfileCoverageReport::evaluate(profile, &mcp_tools, observed_capabilities.as_ref())
    }
}

impl Default for EvaluateContractProfileCoverageUseCase {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use underpass_operator_evaluation_domain::ContractCoverageProfile;

    use super::*;

    #[test]
    fn evaluates_profile_coverage_from_tool_catalog_and_observed_capabilities() {
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

        let report = EvaluateContractProfileCoverageUseCase::new().execute(
            ContractCoverageProfile::Read,
            mcp_tools,
            Some(observed),
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
