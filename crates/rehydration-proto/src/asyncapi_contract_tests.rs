const ASYNCAPI_CONTRACT: &str =
    include_str!("../../../api/asyncapi/context-projection.v1alpha1.yaml");

#[test]
fn asyncapi_exposes_generic_kernel_subjects() {
    assert!(ASYNCAPI_CONTRACT.contains("graph.node.materialized:"));
    assert!(ASYNCAPI_CONTRACT.contains("node.detail.materialized:"));
    assert!(ASYNCAPI_CONTRACT.contains("context.bundle.generated:"));
}

#[test]
fn bundle_generation_event_uses_root_node_id() {
    assert!(ASYNCAPI_CONTRACT.contains("root_node_id:"));
    assert!(!ASYNCAPI_CONTRACT.contains("case_id:"));
}
