use std::io;

use rehydration_proto::v1alpha1::GraphNode;
use serde_json::json;

use crate::runtime_contract::RuntimeResult;

pub fn work_item_candidates(neighbors: &[GraphNode]) -> Vec<serde_json::Value> {
    neighbors
        .iter()
        .filter(|node| node.node_kind == "work_item")
        .map(|node| {
            json!({
                "node_id": node.node_id,
                "title": node.title,
                "status": node.status,
                "sequence": node.properties.get("sequence"),
                "deliverables": node.properties.get("deliverables"),
            })
        })
        .collect()
}

pub fn allowed_step_ids(work_items: &[serde_json::Value]) -> Vec<&str> {
    work_items
        .iter()
        .filter_map(|item| item.get("node_id").and_then(|value| value.as_str()))
        .collect()
}

pub fn build_selection_prompt(work_items: &[serde_json::Value]) -> io::Result<String> {
    let work_items_json = serde_json::to_string(work_items).map_err(io::Error::other)?;
    let allowed_ids_json =
        serde_json::to_string(&allowed_step_ids(work_items)).map_err(io::Error::other)?;
    Ok(format!(
        "Choose the single current step for the mission from these work items: {work_items_json}\nAllowed node ids: {allowed_ids_json}\nRules: prefer IN_PROGRESS; otherwise choose the first non-completed step by numeric sequence.\nReturn exactly one JSON object like {{\"selected_step_node_id\":\"one-of-the-allowed-node-ids\"}}."
    ))
}

pub fn ensure_supported_selection(
    selected_step_node_id: &str,
    allowed_ids: &[&str],
) -> RuntimeResult<()> {
    if allowed_ids.contains(&selected_step_node_id) {
        return Ok(());
    }

    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        format!("LLM selected unsupported step `{selected_step_node_id}`"),
    )
    .into())
}

pub fn deterministic_current_step_node_id(neighbors: &[GraphNode]) -> RuntimeResult<String> {
    let mut work_items = neighbors
        .iter()
        .filter(|node| node.node_kind == "work_item")
        .collect::<Vec<_>>();
    work_items.sort_by_key(|node| sequence_of(node));

    if let Some(node) = work_items
        .iter()
        .find(|node| node.status.eq_ignore_ascii_case("IN_PROGRESS"))
    {
        return Ok(node.node_id.clone());
    }

    if let Some(node) = work_items
        .iter()
        .find(|node| !node.status.eq_ignore_ascii_case("COMPLETED"))
    {
        return Ok(node.node_id.clone());
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "no selectable work_item nodes available",
    )
    .into())
}

fn sequence_of(node: &GraphNode) -> u32 {
    node.properties
        .get("sequence")
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::{
        build_selection_prompt, deterministic_current_step_node_id, ensure_supported_selection,
        sequence_of, work_item_candidates,
    };
    use std::collections::HashMap;

    use rehydration_proto::v1alpha1::GraphNode;

    fn work_item(node_id: &str, status: &str, sequence: &str, deliverables: &str) -> GraphNode {
        GraphNode {
            node_id: node_id.to_string(),
            node_kind: "work_item".to_string(),
            title: format!("Title for {node_id}"),
            summary: String::new(),
            status: status.to_string(),
            labels: Vec::new(),
            properties: HashMap::from([
                ("sequence".to_string(), sequence.to_string()),
                ("deliverables".to_string(), deliverables.to_string()),
            ]),
        }
    }

    #[test]
    fn work_item_candidates_extract_only_work_items() {
        let nodes = vec![
            work_item("node:one", "IN_PROGRESS", "1", "a.rs"),
            GraphNode {
                node_id: "node:other".to_string(),
                node_kind: "note".to_string(),
                title: "Other".to_string(),
                summary: String::new(),
                status: "ACTIVE".to_string(),
                labels: Vec::new(),
                properties: HashMap::new(),
            },
        ];

        let items = work_item_candidates(&nodes);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["node_id"], "node:one");
    }

    #[test]
    fn selection_prompt_mentions_allowed_ids_and_rules() {
        let work_items = work_item_candidates(&[work_item("node:one", "IN_PROGRESS", "1", "a.rs")]);

        let prompt = build_selection_prompt(&work_items).expect("prompt should build");

        assert!(prompt.contains("node:one"));
        assert!(prompt.contains("prefer IN_PROGRESS"));
    }

    #[test]
    fn ensure_supported_selection_rejects_unknown_ids() {
        let error = ensure_supported_selection("node:two", &["node:one"])
            .expect_err("unknown ids must fail");

        assert!(error.to_string().contains("unsupported step"));
    }

    #[test]
    fn deterministic_selection_prefers_in_progress_then_sequence() {
        let nodes = vec![
            work_item("node:two", "PENDING", "2", "b.rs"),
            work_item("node:one", "IN_PROGRESS", "1", "a.rs"),
        ];
        assert_eq!(
            deterministic_current_step_node_id(&nodes).expect("selection should succeed"),
            "node:one"
        );

        let completed_nodes = vec![
            work_item("node:done", "COMPLETED", "1", "a.rs"),
            work_item("node:pending", "PENDING", "2", "b.rs"),
        ];
        assert_eq!(
            deterministic_current_step_node_id(&completed_nodes)
                .expect("fallback selection should succeed"),
            "node:pending"
        );
    }

    #[test]
    fn sequence_helper_parses_expected_values() {
        let node = work_item("node:one", "IN_PROGRESS", "7", "a.rs,b.rs");

        assert_eq!(sequence_of(&node), 7);
    }
}
