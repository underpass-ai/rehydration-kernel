use std::collections::BTreeSet;
use std::io;

use rehydration_proto::v1alpha1::GraphNode;

use crate::CAPTAINS_LOG_PATH;

pub fn parse_deliverables(node: &GraphNode) -> Vec<String> {
    node.properties
        .get("deliverables")
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|entry| !entry.is_empty())
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

pub fn listed_paths(listed_files: &str) -> BTreeSet<String> {
    listed_files
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
        .collect()
}

pub fn pending_deliverables<'a>(
    deliverables: &'a [String],
    existing_paths: &BTreeSet<String>,
) -> Vec<&'a str> {
    deliverables
        .iter()
        .filter(|deliverable| !existing_paths.contains(*deliverable))
        .map(String::as_str)
        .collect()
}

pub fn build_file_generation_prompt(
    root_title: &str,
    step: &GraphNode,
    detail: &str,
    deliverable: &str,
    deliverables: &[String],
    listed_files: &str,
    rendered_content: &str,
) -> io::Result<String> {
    Ok(format!(
        "Mission root: {}\nCurrent step id: {}\nCurrent step title: {}\nCurrent step detail: {}\nTarget deliverable path: {}\nAllowed deliverables for this step: {}\nExisting files:\n{}\nRendered context:\n{}\n\nReturn exactly one JSON object like {{\"content\":\"...\"}}. The content must be compact, plausible, and only for the requested path. Do not include explanations, markdown fences, or any other keys.",
        root_title,
        step.node_id,
        step.title,
        detail,
        deliverable,
        serde_json::to_string(deliverables).map_err(io::Error::other)?,
        listed_files,
        rendered_content,
    ))
}

pub fn should_read_captains_log(written_paths: &[String]) -> bool {
    written_paths.iter().any(|path| path == CAPTAINS_LOG_PATH)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use rehydration_proto::v1alpha1::GraphNode;

    use super::{
        build_file_generation_prompt, listed_paths, parse_deliverables, pending_deliverables,
        should_read_captains_log,
    };

    fn work_item(node_id: &str, deliverables: &str) -> GraphNode {
        GraphNode {
            node_id: node_id.to_string(),
            node_kind: "work_item".to_string(),
            title: format!("Title for {node_id}"),
            summary: String::new(),
            status: "IN_PROGRESS".to_string(),
            labels: Vec::new(),
            properties: HashMap::from([("deliverables".to_string(), deliverables.to_string())]),
        }
    }

    #[test]
    fn deliverable_helpers_parse_expected_values() {
        let node = work_item("node:one", "a.rs,b.rs");

        assert_eq!(
            parse_deliverables(&node),
            vec!["a.rs".to_string(), "b.rs".to_string()]
        );
    }

    #[test]
    fn file_prompt_and_pending_deliverables_use_existing_files() {
        let node = work_item("node:one", "src/a.rs,src/b.rs");
        let deliverables = parse_deliverables(&node);
        let existing_paths = listed_paths("src/a.rs\n");

        assert_eq!(
            pending_deliverables(&deliverables, &existing_paths),
            vec!["src/b.rs"]
        );
        assert!(should_read_captains_log(&["captains-log.md".to_string()]));

        let prompt = build_file_generation_prompt(
            "Repair The Starship",
            &node,
            "detail",
            "src/b.rs",
            &deliverables,
            "src/a.rs",
            "rendered context",
        )
        .expect("prompt should build");
        assert!(prompt.contains("Repair The Starship"));
        assert!(prompt.contains("src/b.rs"));
        assert!(prompt.contains("rendered context"));
    }
}
