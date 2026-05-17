mod action;
mod action_contract;
mod capability;
mod dataset_metrics;
mod error;
mod mode;
mod relation;
mod trajectory;
mod value_objects;

pub use action::{
    ActionArguments, AnswerPolicy, KernelTool, OperatorAction, PreparedPayloadSource, StopAction,
    ToolCallAction,
};
pub use action_contract::{
    OperatorActionContractDiagnostic, OperatorActionContractViolation,
    OperatorActionContractViolationPhase, operator_action_contract_diagnostic,
    operator_action_contract_error, operator_action_shape_error, operator_allowed_full_tools,
    operator_allowed_read_tools, operator_allowed_tools_for_mode, operator_allowed_write_tools,
    operator_allowed_writer_pre_read_tools, operator_is_bounded_tool_call,
    operator_is_valid_action_shape, operator_primary_refs,
    operator_tool_call_arguments_contract_diagnostic, operator_tool_call_arguments_contract_error,
};
pub use capability::KmpMcpCapability;
pub use dataset_metrics::TrainingDatasetPreflightReport;
pub use error::{DomainError, DomainResult};
pub use mode::{
    AllowedTools, OperatorMode, allowed_tool_names_for_mode, full_tool_names,
    parse_allowed_tools_for_mode, read_tool_names, validate_allowed_tools_for_mode,
    write_tool_names, writer_context_read_tool_names,
};
pub use relation::{
    KnownMemoryRelationType, MemoryRelationQuality, MemoryRelationSpec, MemoryRelationType,
    RelationSemanticClass,
};
pub use trajectory::{TrainingTrajectory, VisibleState};
pub use value_objects::{
    AboutId, ArtifactUri, DatasetId, ExampleCount, MemoryRef, ModelId, NonEmptyString,
    PositiveCount, StepId, SyntheticCaseId, TaskFamily, TrainingRunId,
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
            "underpass-operator-shared-domain must stay independent from kernel crates"
        );
    }
}
