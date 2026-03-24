use crate::v1beta1;
use prost::Message;
use prost_types::{
    DescriptorProto, FileDescriptorProto, FileDescriptorSet, ServiceDescriptorProto,
};

const KERNEL_PACKAGE: &str = "underpass.rehydration.kernel.v1beta1";

#[test]
fn v1beta1_query_service_surface_is_stable() {
    let descriptor_set = decode_kernel_descriptor_set();
    let query_file = kernel_file(&descriptor_set, "query.proto");

    assert_eq!(
        service_method_names(query_file, "ContextQueryService"),
        vec![
            "GetContext",
            "GetContextPath",
            "GetNodeDetail",
            "RehydrateSession",
            "ValidateScope"
        ]
    );
}

#[test]
fn v1beta1_command_service_surface_is_stable() {
    let descriptor_set = decode_kernel_descriptor_set();
    let command_file = kernel_file(&descriptor_set, "command.proto");

    assert_eq!(
        service_method_names(command_file, "ContextCommandService"),
        vec!["UpdateContext"]
    );
}

#[test]
fn v1beta1_graph_relationship_fields_are_stable() {
    let descriptor_set = decode_kernel_descriptor_set();
    let common_file = kernel_file(&descriptor_set, "common.proto");

    assert_eq!(
        message_field_names(common_file, "GraphRelationship"),
        vec![
            "source_node_id",
            "target_node_id",
            "relationship_type",
            "explanation",
        ]
    );
    assert_eq!(
        message_field_names(common_file, "GraphRelationshipExplanation"),
        vec![
            "semantic_class",
            "rationale",
            "motivation",
            "method",
            "decision_id",
            "caused_by_node_id",
            "evidence",
            "confidence",
            "sequence",
        ]
    );
}

fn decode_kernel_descriptor_set() -> FileDescriptorSet {
    FileDescriptorSet::decode(v1beta1::FILE_DESCRIPTOR_SET)
        .expect("kernel v1beta1 descriptor set should decode")
}

fn kernel_file<'a>(
    descriptor_set: &'a FileDescriptorSet,
    file_name: &str,
) -> &'a FileDescriptorProto {
    let full_name = format!("underpass/rehydration/kernel/v1beta1/{file_name}");

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
