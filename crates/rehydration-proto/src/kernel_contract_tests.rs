use crate::v1alpha1;
use prost::Message;
use prost_types::{
    DescriptorProto, FileDescriptorProto, FileDescriptorSet, ServiceDescriptorProto,
};

const KERNEL_PACKAGE: &str = "underpass.rehydration.kernel.v1alpha1";

#[test]
fn query_service_surface_is_stable() {
    let descriptor_set = decode_kernel_descriptor_set();
    let query_file = kernel_file(&descriptor_set, "query.proto");

    assert_eq!(
        service_method_names(query_file, "ContextQueryService"),
        vec!["GetContext", "RehydrateSession", "ValidateScope"]
    );
}

#[test]
fn command_service_surface_is_stable() {
    let descriptor_set = decode_kernel_descriptor_set();
    let command_file = kernel_file(&descriptor_set, "command.proto");

    assert_eq!(
        service_method_names(command_file, "ContextCommandService"),
        vec!["UpdateContext"]
    );
}

#[test]
fn admin_service_surface_is_stable() {
    let descriptor_set = decode_kernel_descriptor_set();
    let admin_file = kernel_file(&descriptor_set, "admin.proto");

    assert_eq!(
        service_method_names(admin_file, "ContextAdminService"),
        vec![
            "GetProjectionStatus",
            "ReplayProjection",
            "GetBundleSnapshot",
            "GetGraphRelationships",
            "GetRehydrationDiagnostics",
        ]
    );
}

#[test]
fn get_context_request_fields_are_stable() {
    let descriptor_set = decode_kernel_descriptor_set();
    let query_file = kernel_file(&descriptor_set, "query.proto");

    assert_eq!(
        message_field_names(query_file, "GetContextRequest"),
        vec![
            "root_node_id",
            "role",
            "phase",
            "work_item_id",
            "token_budget",
            "requested_scopes",
            "render_format",
            "include_debug_sections",
        ]
    );
}

#[test]
fn rehydration_bundle_fields_are_stable() {
    let descriptor_set = decode_kernel_descriptor_set();
    let common_file = kernel_file(&descriptor_set, "common.proto");

    assert_eq!(
        message_field_names(common_file, "RehydrationBundle"),
        vec!["root_node_id", "bundles", "stats", "version"]
    );
}

#[test]
fn graph_relationships_request_fields_are_stable() {
    let descriptor_set = decode_kernel_descriptor_set();
    let admin_file = kernel_file(&descriptor_set, "admin.proto");

    assert_eq!(
        message_field_names(admin_file, "GetGraphRelationshipsRequest"),
        vec!["node_id", "node_kind", "depth", "include_reverse_edges"]
    );
}

fn decode_kernel_descriptor_set() -> FileDescriptorSet {
    FileDescriptorSet::decode(v1alpha1::FILE_DESCRIPTOR_SET)
        .expect("kernel descriptor set should decode")
}

fn kernel_file<'a>(
    descriptor_set: &'a FileDescriptorSet,
    file_name: &str,
) -> &'a FileDescriptorProto {
    let full_name = format!("underpass/rehydration/kernel/v1alpha1/{file_name}");

    descriptor_set
        .file
        .iter()
        .find(|file| {
            file.package.as_deref() == Some(KERNEL_PACKAGE)
                && file.name.as_deref() == Some(full_name.as_str())
        })
        .unwrap_or_else(|| panic!("missing descriptor file `{full_name}`"))
}

fn service_method_names(file: &FileDescriptorProto, service_name: &str) -> Vec<String> {
    file.service
        .iter()
        .find(|service| service.name.as_deref() == Some(service_name))
        .unwrap_or_else(|| panic!("missing service `{service_name}`"))
        .method
        .iter()
        .map(|method| method.name.clone().expect("method name should be present"))
        .collect()
}

fn message_field_names(file: &FileDescriptorProto, message_name: &str) -> Vec<String> {
    file.message_type
        .iter()
        .find(|message| message.name.as_deref() == Some(message_name))
        .unwrap_or_else(|| panic!("missing message `{message_name}`"))
        .field
        .iter()
        .map(|field| field.name.clone().expect("field name should be present"))
        .collect()
}

#[allow(dead_code)]
fn _service_names(file: &FileDescriptorProto) -> Vec<String> {
    file.service.iter().map(service_name).collect()
}

#[allow(dead_code)]
fn _message_names(file: &FileDescriptorProto) -> Vec<String> {
    file.message_type.iter().map(message_name).collect()
}

fn service_name(service: &ServiceDescriptorProto) -> String {
    service
        .name
        .clone()
        .expect("service name should be present")
}

fn message_name(message: &DescriptorProto) -> String {
    message
        .name
        .clone()
        .expect("message name should be present")
}
