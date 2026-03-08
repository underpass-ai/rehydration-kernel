use std::collections::BTreeMap;

use rehydration_domain::{BundleNode, BundleNodeDetail, BundleRelationship, RehydrationBundle};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedContext {
    pub content: String,
    pub token_count: u32,
    pub sections: Vec<String>,
}

pub fn render_graph_bundle(bundle: &RehydrationBundle) -> RenderedContext {
    let detail_by_node_id = bundle
        .node_details()
        .iter()
        .map(|detail| (detail.node_id(), detail))
        .collect::<BTreeMap<_, _>>();

    let mut sections = Vec::new();
    sections.push(render_node(bundle.root_node()));

    for node in bundle.neighbor_nodes() {
        sections.push(render_node(node));
    }

    for relationship in bundle.relationships() {
        sections.push(render_relationship(relationship));
    }

    for detail in bundle.node_details() {
        sections.push(render_detail(detail, &detail_by_node_id));
    }

    let content = sections.join("\n\n");
    let token_count = content.split_whitespace().count() as u32;

    RenderedContext {
        content,
        token_count,
        sections,
    }
}

fn render_node(node: &BundleNode) -> String {
    let mut section = format!("Node {} ({})", node.title(), node.node_kind());
    if !node.summary().trim().is_empty() {
        section.push_str(": ");
        section.push_str(node.summary().trim());
    }
    section
}

fn render_relationship(relationship: &BundleRelationship) -> String {
    format!(
        "Relationship {} --{}--> {}",
        relationship.source_node_id(),
        relationship.relationship_type(),
        relationship.target_node_id()
    )
}

fn render_detail(
    detail: &BundleNodeDetail,
    detail_by_node_id: &BTreeMap<&str, &BundleNodeDetail>,
) -> String {
    let revision = detail_by_node_id
        .get(detail.node_id())
        .map(|value| value.revision())
        .unwrap_or(detail.revision());

    format!(
        "Detail {} [rev {}]: {}",
        detail.node_id(),
        revision,
        detail.detail()
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use rehydration_domain::{
        BundleMetadata, BundleNode, BundleNodeDetail, BundleRelationship, CaseId,
        RehydrationBundle, Role,
    };

    use super::render_graph_bundle;

    #[test]
    fn render_graph_bundle_orders_root_neighbors_relationships_and_details() {
        let bundle = RehydrationBundle::new(
            CaseId::new("case-123").expect("case id is valid"),
            Role::new("developer").expect("role is valid"),
            BundleNode::new(
                "case-123",
                "case",
                "Root",
                "Root summary",
                "ACTIVE",
                vec![],
                BTreeMap::new(),
            ),
            vec![BundleNode::new(
                "node-1",
                "decision",
                "Neighbor",
                "Neighbor summary",
                "ACTIVE",
                vec![],
                BTreeMap::new(),
            )],
            vec![BundleRelationship::new(
                "case-123",
                "node-1",
                "RELATES_TO",
                BTreeMap::new(),
            )],
            vec![BundleNodeDetail::new(
                "case-123",
                "Expanded detail",
                "hash-1",
                2,
            )],
            BundleMetadata::initial("0.1.0"),
        )
        .expect("bundle should be valid");

        let rendered = render_graph_bundle(&bundle);

        assert_eq!(rendered.sections.len(), 4);
        assert!(rendered.sections[0].starts_with("Node Root"));
        assert!(rendered.sections[1].starts_with("Node Neighbor"));
        assert!(rendered.sections[2].starts_with("Relationship"));
        assert!(rendered.sections[3].starts_with("Detail case-123"));
    }
}
