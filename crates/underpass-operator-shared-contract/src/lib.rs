mod action;
mod trajectory;

pub use action::{OperatorActionDto, PreparedToolCallActionDto, StopActionDto, ToolCallActionDto};
pub use trajectory::RawTrajectoryDto;

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
            "underpass-operator-shared-contract must stay independent from kernel crates"
        );
    }
}
