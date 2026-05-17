mod errors;
pub mod ports;
pub mod use_cases;

pub use errors::{
    GenerateSyntheticCaseError, SyntheticApplicationError, SyntheticApplicationResult,
};
pub use use_cases::{
    BuildOperatorTrajectoryUseCase, GenerateSyntheticDatasetUseCase,
    PlanKmpMcpSyntheticCasesUseCase,
};

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
            "underpass-operator-synthetic-application must stay independent from kernel crates"
        );
    }
}
