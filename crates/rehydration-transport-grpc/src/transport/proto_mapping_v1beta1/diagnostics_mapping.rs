use rehydration_application::{GetRehydrationDiagnosticsResult, RehydrationDiagnosticView};
use rehydration_proto::v1beta1::{GetRehydrationDiagnosticsResponse, RehydrationDiagnostic};

use crate::transport::proto_mapping_v1beta1::proto_bundle_version_v1beta1;
use crate::transport::support::timestamp_from;

pub(crate) fn proto_rehydration_diagnostics_response_v1beta1(
    result: &GetRehydrationDiagnosticsResult,
) -> GetRehydrationDiagnosticsResponse {
    GetRehydrationDiagnosticsResponse {
        diagnostics: result
            .diagnostics
            .iter()
            .map(proto_diagnostic_v1beta1)
            .collect(),
        observed_at: Some(timestamp_from(result.observed_at)),
    }
}

pub(crate) fn proto_diagnostic_v1beta1(
    diagnostic: &RehydrationDiagnosticView,
) -> RehydrationDiagnostic {
    RehydrationDiagnostic {
        role: diagnostic.role.clone(),
        version: Some(proto_bundle_version_v1beta1(&diagnostic.version)),
        selected_nodes: diagnostic.selected_nodes,
        selected_relationships: diagnostic.selected_relationships,
        detailed_nodes: diagnostic.detailed_nodes,
        estimated_tokens: diagnostic.estimated_tokens,
        notes: diagnostic.notes.clone(),
    }
}
