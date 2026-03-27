//! Raw document dump baseline for token efficiency comparison.
//!
//! Produces a flat text representation of the same graph data that the
//! kernel renders as structured bundles. Comparing token counts between
//! the two measures the kernel's compression efficiency.

use tiktoken_rs::cl100k_base;

use crate::dataset_generator::GeneratedSeed;

/// Render all seed data as flat unstructured text.
///
/// Contains the same information as the kernel's structured rendering
/// but without hierarchy, semantic grouping, or salience ordering.
pub fn render_raw_dump(seed: &GeneratedSeed) -> String {
    let mut out = String::new();

    // Root node
    out.push_str(&format!(
        "Node: {}. Kind: {}. Summary: {}.",
        seed.root.node_id, seed.root.node_kind, seed.root.summary
    ));
    if let Some(detail) = &seed.root.detail {
        out.push_str(&format!(" Detail: {detail}."));
    }
    out.push('\n');

    // All other nodes
    for node in &seed.nodes {
        out.push_str(&format!(
            "Node: {}. Kind: {}. Summary: {}.",
            node.node_id, node.node_kind, node.summary
        ));
        if let Some(detail) = &node.detail {
            out.push_str(&format!(" Detail: {detail}."));
        }
        out.push('\n');
    }

    // All relationships
    for rel in &seed.relations {
        out.push_str(&format!(
            "Relationship: {} connects to {} via {}. Semantic class: {}.",
            rel.source_node_id,
            rel.target_node_id,
            rel.relation_type,
            rel.semantic_class.as_str(),
        ));
        if let Some(rationale) = &rel.rationale {
            out.push_str(&format!(" Rationale: {rationale}."));
        }
        if let Some(motivation) = &rel.motivation {
            out.push_str(&format!(" Motivation: {motivation}."));
        }
        if let Some(method) = &rel.method {
            out.push_str(&format!(" Method: {method}."));
        }
        if let Some(decision_id) = &rel.decision_id {
            out.push_str(&format!(" Decision: {decision_id}."));
        }
        if let Some(caused_by) = &rel.caused_by_node_id {
            out.push_str(&format!(" Caused by: {caused_by}."));
        }
        out.push('\n');
    }

    out
}

/// Count tokens using cl100k_base (same tokenizer as the kernel).
pub fn count_tokens(text: &str) -> usize {
    let bpe = cl100k_base().expect("cl100k_base tokenizer should load");
    bpe.encode_ordinary(text).len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataset_generator::{Domain, GraphSeedConfig, generate_seed};

    #[test]
    fn raw_dump_contains_all_nodes() {
        let seed = generate_seed(GraphSeedConfig::micro(Domain::Operations));
        let dump = render_raw_dump(&seed);

        // Root + 3 chain nodes = 4 "Node:" lines
        let node_count = dump.matches("Node:").count();
        assert_eq!(node_count, 4, "raw dump should contain all 4 nodes");
    }

    #[test]
    fn raw_dump_contains_all_relationships() {
        let seed = generate_seed(GraphSeedConfig::micro(Domain::Operations));
        let dump = render_raw_dump(&seed);

        let rel_count = dump.matches("Relationship:").count();
        assert_eq!(rel_count, 3, "raw dump should contain all 3 relationships");
    }

    #[test]
    fn raw_dump_token_count_is_positive() {
        let seed = generate_seed(GraphSeedConfig::micro(Domain::Operations));
        let dump = render_raw_dump(&seed);
        let tokens = count_tokens(&dump);

        assert!(tokens > 50, "raw dump should have significant token count, got {tokens}");
    }

    #[test]
    fn raw_dump_is_larger_than_structured_for_meso() {
        // The raw dump should use more tokens than a structured rendering
        // would for the same data, because it has no compression.
        let seed = generate_seed(GraphSeedConfig::meso(Domain::Operations));
        let dump = render_raw_dump(&seed);
        let raw_tokens = count_tokens(&dump);

        // Meso has 21 nodes and 20 relations — raw dump should be substantial
        assert!(raw_tokens > 200, "meso raw dump should be >200 tokens, got {raw_tokens}");
    }
}
