mod bundle_mapping;
mod diagnostics_mapping;
mod graph_mapping;
mod projection_mapping;
mod rendered_mapping;
mod scope_mapping;
mod version_mapping;

pub(crate) use bundle_mapping::{
    proto_bundle_from_single_role, proto_bundle_snapshot_response, proto_rehydrate_session_response,
};
pub(crate) use diagnostics_mapping::proto_rehydration_diagnostics_response;
pub(crate) use graph_mapping::{
    proto_bundle_node, proto_bundle_node_detail, proto_bundle_relationship, proto_graph_node,
    proto_graph_relationships_response, proto_node_detail_view,
};
pub(crate) use projection_mapping::{
    proto_projection_status_response, proto_replay_projection_response,
};
pub(crate) use rendered_mapping::proto_rendered_context_from_result;
pub(crate) use scope_mapping::proto_scope_validation;
pub(crate) use version_mapping::{proto_accepted_version, proto_bundle_version};

#[cfg(test)]
pub(crate) use diagnostics_mapping::proto_diagnostic;
#[cfg(test)]
pub(crate) use graph_mapping::proto_graph_relationship;
#[cfg(test)]
pub(crate) use projection_mapping::proto_projection_status;
