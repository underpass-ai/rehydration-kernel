use serde_json::Value;

const PROMPT: &str =
    include_str!("../../../api/examples/inference-prompts/kernel-write-memory.txt");
const REQUEST: &str =
    include_str!("../../../api/examples/inference-prompts/kernel-write-memory.request.json");

#[test]
fn kernel_write_memory_prompt_fixture_is_schema_constrained() {
    assert!(PROMPT.contains("kernel_write_memory"));
    assert!(PROMPT.contains("read_context"));
    assert!(PROMPT.contains("Do not use vague relations"));

    let request: Value = serde_json::from_str(REQUEST)
        .expect("kernel write memory request fixture should be valid JSON");
    assert_eq!(
        request["response_format"]["json_schema"]["name"],
        "kernel_write_memory_arguments"
    );

    let schema = &request["response_format"]["json_schema"]["schema"];
    assert_eq!(schema["type"], "object");
    assert!(
        schema["required"]
            .as_array()
            .expect("schema required should be an array")
            .iter()
            .any(|field| field == "read_context")
    );
    assert!(schema["properties"].get("connect_to").is_some());
    assert!(schema["properties"].get("read_context").is_some());

    let rel_enum = schema["$defs"]["write_link"]["properties"]["rel"]["enum"]
        .as_array()
        .expect("relation enum should be an array");
    assert!(rel_enum.iter().any(|rel| rel == "chosen_because"));
    assert!(!rel_enum.iter().any(|rel| rel == "related_to"));
}
