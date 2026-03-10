use crate::v1alpha1;
use prost::Message;
use prost_types::{DescriptorProto, FieldDescriptorProto, FileDescriptorProto, FileDescriptorSet};
use serde_json::Value;

const KERNEL_PACKAGE: &str = "underpass.rehydration.kernel.v1alpha1";
const GET_CONTEXT_REQUEST_FIXTURE: &str =
    include_str!("../../../api/examples/kernel/v1alpha1/grpc/get-context.request.json");
const GET_CONTEXT_RESPONSE_FIXTURE: &str =
    include_str!("../../../api/examples/kernel/v1alpha1/grpc/get-context.response.json");
const REHYDRATE_SESSION_REQUEST_FIXTURE: &str =
    include_str!("../../../api/examples/kernel/v1alpha1/grpc/rehydrate-session.request.json");
const UPDATE_CONTEXT_REQUEST_FIXTURE: &str =
    include_str!("../../../api/examples/kernel/v1alpha1/grpc/update-context.request.json");
const GET_GRAPH_RELATIONSHIPS_REQUEST_FIXTURE: &str =
    include_str!("../../../api/examples/kernel/v1alpha1/grpc/get-graph-relationships.request.json");
const GET_GRAPH_RELATIONSHIPS_RESPONSE_FIXTURE: &str = include_str!(
    "../../../api/examples/kernel/v1alpha1/grpc/get-graph-relationships.response.json"
);
const GRAPH_NODE_MATERIALIZED_FIXTURE: &str =
    include_str!("../../../api/examples/kernel/v1alpha1/async/graph.node.materialized.json");
const NODE_DETAIL_MATERIALIZED_FIXTURE: &str =
    include_str!("../../../api/examples/kernel/v1alpha1/async/node.detail.materialized.json");
const CONTEXT_BUNDLE_GENERATED_FIXTURE: &str =
    include_str!("../../../api/examples/kernel/v1alpha1/async/context.bundle.generated.json");

#[test]
fn grpc_reference_fixtures_match_protojson_contract() {
    let descriptor_set = decode_kernel_descriptor_set();

    assert_fixture_keys_match_message(
        &descriptor_set,
        "GetContextRequest",
        &parse_fixture(GET_CONTEXT_REQUEST_FIXTURE),
    );
    assert_fixture_keys_match_message(
        &descriptor_set,
        "GetContextResponse",
        &parse_fixture(GET_CONTEXT_RESPONSE_FIXTURE),
    );
    assert_fixture_keys_match_message(
        &descriptor_set,
        "RehydrateSessionRequest",
        &parse_fixture(REHYDRATE_SESSION_REQUEST_FIXTURE),
    );
    assert_fixture_keys_match_message(
        &descriptor_set,
        "UpdateContextRequest",
        &parse_fixture(UPDATE_CONTEXT_REQUEST_FIXTURE),
    );
    assert_fixture_keys_match_message(
        &descriptor_set,
        "GetGraphRelationshipsRequest",
        &parse_fixture(GET_GRAPH_RELATIONSHIPS_REQUEST_FIXTURE),
    );
    assert_fixture_keys_match_message(
        &descriptor_set,
        "GetGraphRelationshipsResponse",
        &parse_fixture(GET_GRAPH_RELATIONSHIPS_RESPONSE_FIXTURE),
    );
}

#[test]
fn grpc_response_reference_fixture_uses_generic_bundle_shape() {
    let descriptor_set = decode_kernel_descriptor_set();
    let fixture = parse_fixture(GET_CONTEXT_RESPONSE_FIXTURE);
    let bundle = fixture_object_field(&fixture, "bundle");
    let rendered = fixture_object_field(&fixture, "rendered");
    let scope_validation = fixture_object_field(&fixture, "scopeValidation");
    let first_role_bundle = fixture_array_field(bundle, "bundles")
        .first()
        .expect("bundle fixture should contain one role bundle");

    assert_fixture_keys_match_message(&descriptor_set, "RehydrationBundle", bundle);
    assert_fixture_keys_match_message(&descriptor_set, "RenderedContext", rendered);
    assert_fixture_keys_match_message(&descriptor_set, "ScopeValidationResult", scope_validation);
    assert_fixture_keys_match_message(&descriptor_set, "GraphRoleBundle", first_role_bundle);
    assert!(!collect_object_keys(bundle).contains(&"caseId".to_string()));
}

#[test]
fn async_reference_fixtures_use_generic_event_envelope_and_node_centric_payloads() {
    assert_async_fixture_matches_expected_shape(
        parse_fixture(GRAPH_NODE_MATERIALIZED_FIXTURE),
        &[
            "event_id",
            "correlation_id",
            "causation_id",
            "occurred_at",
            "aggregate_id",
            "aggregate_type",
            "schema_version",
            "data",
        ],
        &[
            "node_id",
            "node_kind",
            "title",
            "summary",
            "status",
            "labels",
            "properties",
            "related_nodes",
        ],
    );
    assert_async_fixture_matches_expected_shape(
        parse_fixture(NODE_DETAIL_MATERIALIZED_FIXTURE),
        &[
            "event_id",
            "correlation_id",
            "causation_id",
            "occurred_at",
            "aggregate_id",
            "aggregate_type",
            "schema_version",
            "data",
        ],
        &["node_id", "detail", "content_hash", "revision"],
    );
    assert_async_fixture_matches_expected_shape(
        parse_fixture(CONTEXT_BUNDLE_GENERATED_FIXTURE),
        &[
            "event_id",
            "correlation_id",
            "causation_id",
            "occurred_at",
            "aggregate_id",
            "aggregate_type",
            "schema_version",
            "data",
        ],
        &[
            "root_node_id",
            "roles",
            "revision",
            "content_hash",
            "projection_watermark",
        ],
    );
}

#[test]
fn reference_fixtures_do_not_reintroduce_legacy_product_nouns() {
    let fixtures = [
        parse_fixture(GET_CONTEXT_REQUEST_FIXTURE),
        parse_fixture(GET_CONTEXT_RESPONSE_FIXTURE),
        parse_fixture(REHYDRATE_SESSION_REQUEST_FIXTURE),
        parse_fixture(UPDATE_CONTEXT_REQUEST_FIXTURE),
        parse_fixture(GET_GRAPH_RELATIONSHIPS_REQUEST_FIXTURE),
        parse_fixture(GET_GRAPH_RELATIONSHIPS_RESPONSE_FIXTURE),
        parse_fixture(GRAPH_NODE_MATERIALIZED_FIXTURE),
        parse_fixture(NODE_DETAIL_MATERIALIZED_FIXTURE),
        parse_fixture(CONTEXT_BUNDLE_GENERATED_FIXTURE),
    ];

    let legacy_keys = [
        "case_id", "caseId", "story_id", "storyId", "task_id", "taskId",
    ];

    for fixture in fixtures {
        let keys = collect_object_keys(&fixture);

        for legacy_key in legacy_keys {
            assert!(
                !keys.iter().any(|key| key == legacy_key),
                "fixture should not contain legacy key `{legacy_key}`"
            );
        }
    }
}

fn assert_async_fixture_matches_expected_shape(
    fixture: Value,
    expected_envelope_keys: &[&str],
    expected_data_keys: &[&str],
) {
    fixture
        .as_object()
        .expect("async fixture should be a JSON object");
    let data = fixture_object_field(&fixture, "data");

    assert_eq!(
        sorted_keys(object_keys(&fixture)),
        sorted_strs(expected_envelope_keys)
    );
    assert_eq!(
        sorted_keys(object_keys(data)),
        sorted_strs(expected_data_keys)
    );
}

fn assert_fixture_keys_match_message(
    descriptor_set: &FileDescriptorSet,
    message_name: &str,
    fixture: &Value,
) {
    assert_eq!(
        sorted_keys(object_keys(fixture)),
        sorted_keys(message_json_field_names(descriptor_set, message_name)),
        "fixture keys should match message `{message_name}`"
    );
}

fn parse_fixture(fixture: &str) -> Value {
    serde_json::from_str(fixture).expect("fixture JSON should parse")
}

fn object_keys(value: &Value) -> Vec<String> {
    let object = value.as_object().expect("fixture should be a JSON object");
    object.keys().cloned().collect()
}

fn sorted_keys(mut keys: Vec<String>) -> Vec<String> {
    keys.sort_unstable();
    keys
}

fn sorted_strs(keys: &[&str]) -> Vec<String> {
    let mut keys = keys.iter().map(|key| key.to_string()).collect::<Vec<_>>();
    keys.sort_unstable();
    keys
}

fn fixture_object_field<'a>(value: &'a Value, key: &str) -> &'a Value {
    value
        .get(key)
        .unwrap_or_else(|| panic!("fixture should contain object field `{key}`"))
}

fn fixture_array_field<'a>(value: &'a Value, key: &str) -> &'a Vec<Value> {
    value
        .get(key)
        .unwrap_or_else(|| panic!("fixture should contain array field `{key}`"))
        .as_array()
        .unwrap_or_else(|| panic!("fixture field `{key}` should be an array"))
}

fn collect_object_keys(value: &Value) -> Vec<String> {
    let mut keys = Vec::new();
    collect_object_keys_inner(value, &mut keys);
    keys
}

fn collect_object_keys_inner(value: &Value, keys: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, nested) in map {
                keys.push(key.clone());
                collect_object_keys_inner(nested, keys);
            }
        }
        Value::Array(values) => {
            for nested in values {
                collect_object_keys_inner(nested, keys);
            }
        }
        _ => {}
    }
}

fn message_json_field_names(descriptor_set: &FileDescriptorSet, message_name: &str) -> Vec<String> {
    message_descriptor(descriptor_set, message_name)
        .field
        .iter()
        .map(field_json_name)
        .collect()
}

fn field_json_name(field: &FieldDescriptorProto) -> String {
    field
        .json_name
        .clone()
        .or(field.name.clone())
        .expect("field should have a name")
}

fn message_descriptor<'a>(
    descriptor_set: &'a FileDescriptorSet,
    message_name: &str,
) -> &'a DescriptorProto {
    descriptor_set
        .file
        .iter()
        .filter(|file| file.package.as_deref() == Some(KERNEL_PACKAGE))
        .find_map(|file| {
            file.message_type
                .iter()
                .find(|message| message.name.as_deref() == Some(message_name))
        })
        .unwrap_or_else(|| panic!("missing message descriptor `{message_name}`"))
}

fn decode_kernel_descriptor_set() -> FileDescriptorSet {
    FileDescriptorSet::decode(v1alpha1::FILE_DESCRIPTOR_SET)
        .expect("kernel descriptor set should decode")
}

#[allow(dead_code)]
fn _kernel_files(descriptor_set: &FileDescriptorSet) -> Vec<&FileDescriptorProto> {
    descriptor_set
        .file
        .iter()
        .filter(|file| file.package.as_deref() == Some(KERNEL_PACKAGE))
        .collect()
}
