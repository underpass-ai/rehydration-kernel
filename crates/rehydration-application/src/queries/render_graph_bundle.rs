use std::collections::BTreeMap;

use rehydration_domain::{BundleNode, BundleNodeDetail, BundleRelationship, RehydrationBundle};

use crate::queries::ContextRenderOptions;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedContext {
    pub content: String,
    pub token_count: u32,
    pub sections: Vec<String>,
}

pub fn render_graph_bundle(bundle: &RehydrationBundle) -> RenderedContext {
    render_graph_bundle_with_options(bundle, &ContextRenderOptions::default())
}

pub fn render_graph_bundle_with_options(
    bundle: &RehydrationBundle,
    options: &ContextRenderOptions,
) -> RenderedContext {
    let detail_by_node_id = bundle
        .node_details()
        .iter()
        .map(|detail| (detail.node_id(), detail))
        .collect::<BTreeMap<_, _>>();

    let sections = limit_sections_by_token_budget(
        ordered_sections(bundle, &detail_by_node_id, options),
        options,
    );

    let content = sections.join("\n\n");
    let token_count = content.split_whitespace().count() as u32;

    RenderedContext {
        content,
        token_count,
        sections,
    }
}

fn ordered_sections(
    bundle: &RehydrationBundle,
    detail_by_node_id: &BTreeMap<&str, &BundleNodeDetail>,
    options: &ContextRenderOptions,
) -> Vec<String> {
    let mut sections = Vec::new();
    sections.push(render_node(bundle.root_node()));

    let focus_node_id = focus_node_id(bundle, options);

    if let Some(focus_node_id) = focus_node_id
        && focus_node_id != bundle.root_node().node_id()
        && let Some(node) = bundle
            .neighbor_nodes()
            .iter()
            .find(|node| node.node_id() == focus_node_id)
    {
        sections.push(render_node(node));
    }

    for node in bundle.neighbor_nodes() {
        if Some(node.node_id()) != focus_node_id {
            sections.push(render_node(node));
        }
    }

    for relationship in prioritized_relationships(bundle, focus_node_id) {
        sections.push(render_relationship(relationship));
    }

    for detail in prioritized_details(bundle, focus_node_id) {
        sections.push(render_detail(detail, detail_by_node_id));
    }

    sections
}

fn focus_node_id<'a>(
    bundle: &'a RehydrationBundle,
    options: &'a ContextRenderOptions,
) -> Option<&'a str> {
    let focus_node_id = options.focus_node_id.as_deref()?;
    if bundle.root_node().node_id() == focus_node_id
        || bundle
            .neighbor_nodes()
            .iter()
            .any(|node| node.node_id() == focus_node_id)
    {
        Some(focus_node_id)
    } else {
        None
    }
}

fn prioritized_relationships<'a>(
    bundle: &'a RehydrationBundle,
    focus_node_id: Option<&'a str>,
) -> Vec<&'a BundleRelationship> {
    let Some(focus_node_id) = focus_node_id else {
        return bundle.relationships().iter().collect();
    };

    let (focused, remaining): (Vec<_>, Vec<_>) =
        bundle.relationships().iter().partition(|relationship| {
            relationship.source_node_id() == focus_node_id
                || relationship.target_node_id() == focus_node_id
        });

    focused.into_iter().chain(remaining).collect()
}

fn prioritized_details<'a>(
    bundle: &'a RehydrationBundle,
    focus_node_id: Option<&'a str>,
) -> Vec<&'a BundleNodeDetail> {
    let Some(focus_node_id) = focus_node_id else {
        return bundle.node_details().iter().collect();
    };

    let (focused, remaining): (Vec<_>, Vec<_>) = bundle
        .node_details()
        .iter()
        .partition(|detail| detail.node_id() == focus_node_id);

    focused.into_iter().chain(remaining).collect()
}

fn limit_sections_by_token_budget(
    sections: Vec<String>,
    options: &ContextRenderOptions,
) -> Vec<String> {
    let Some(token_budget) = options.token_budget else {
        return sections;
    };

    let mut limited = Vec::new();
    let mut token_count = 0u32;

    for section in sections {
        let section_tokens = section.split_whitespace().count() as u32;
        if limited.is_empty() || token_count + section_tokens <= token_budget {
            token_count += section_tokens;
            limited.push(section);
        } else {
            break;
        }
    }

    limited
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
    let mut section = format!(
        "Relationship {} --{}--> {}",
        relationship.source_node_id(),
        relationship.relationship_type(),
        relationship.target_node_id()
    );

    section.push_str(" [");
    section.push_str(relationship.explanation().semantic_class().as_str());
    section.push(']');

    if let Some(rationale) = relationship
        .explanation()
        .rationale()
        .or(relationship.explanation().motivation())
    {
        section.push_str(" because ");
        section.push_str(rationale);
    }
    if let Some(method) = relationship.explanation().method() {
        section.push_str(" via ");
        section.push_str(method);
    }
    if let Some(decision_id) = relationship.explanation().decision_id() {
        section.push_str(" decision=");
        section.push_str(decision_id);
    }
    if let Some(caused_by_node_id) = relationship.explanation().caused_by_node_id() {
        section.push_str(" caused_by=");
        section.push_str(caused_by_node_id);
    }
    if let Some(sequence) = relationship.explanation().sequence() {
        section.push_str(" step=");
        section.push_str(&sequence.to_string());
    }

    section
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
        RehydrationBundle, RelationExplanation, RelationSemanticClass, Role,
    };

    use crate::queries::ContextRenderOptions;

    use super::{render_graph_bundle, render_graph_bundle_with_options};

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
                RelationExplanation::new(RelationSemanticClass::Structural),
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

    #[test]
    fn render_graph_bundle_prioritizes_focused_node_sections() {
        let bundle = sample_bundle();

        let rendered = render_graph_bundle_with_options(
            &bundle,
            &ContextRenderOptions {
                focus_node_id: Some("node-2".to_string()),
                token_budget: None,
            },
        );

        assert!(rendered.sections[0].starts_with("Node Root"));
        assert!(rendered.sections[1].starts_with("Node Focused"));
        assert!(rendered.sections[3].contains("node-2"));
    }

    #[test]
    fn render_graph_bundle_respects_token_budget_after_reordering() {
        let bundle = sample_bundle();

        let rendered = render_graph_bundle_with_options(
            &bundle,
            &ContextRenderOptions {
                focus_node_id: Some("node-2".to_string()),
                token_budget: Some(8),
            },
        );

        assert_eq!(rendered.sections.len(), 1);
        assert!(rendered.token_count <= 8);
        assert!(rendered.content.starts_with("Node Root"));
    }

    fn sample_bundle() -> RehydrationBundle {
        RehydrationBundle::new(
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
            vec![
                BundleNode::new(
                    "node-1",
                    "decision",
                    "Neighbor",
                    "Neighbor summary",
                    "ACTIVE",
                    vec![],
                    BTreeMap::new(),
                ),
                BundleNode::new(
                    "node-2",
                    "task",
                    "Focused",
                    "Focused summary",
                    "READY",
                    vec![],
                    BTreeMap::new(),
                ),
            ],
            vec![
                BundleRelationship::new(
                    "case-123",
                    "node-1",
                    "RELATES_TO",
                    RelationExplanation::new(RelationSemanticClass::Structural),
                ),
                BundleRelationship::new(
                    "case-123",
                    "node-2",
                    "HAS_TASK",
                    RelationExplanation::new(RelationSemanticClass::Structural),
                ),
            ],
            vec![
                BundleNodeDetail::new("case-123", "Expanded detail", "hash-1", 2),
                BundleNodeDetail::new("node-2", "Focused detail", "hash-2", 3),
            ],
            BundleMetadata::initial("0.1.0"),
        )
        .expect("bundle should be valid")
    }

    #[test]
    fn render_graph_bundle_includes_explanatory_relation_metadata() {
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
                "task",
                "Neighbor",
                "Neighbor summary",
                "ACTIVE",
                vec![],
                BTreeMap::new(),
            )],
            vec![BundleRelationship::new(
                "case-123",
                "node-1",
                "AUTHORIZES",
                RelationExplanation::new(RelationSemanticClass::Motivational)
                    .with_rationale("reserve power must be diverted before repair")
                    .with_decision_id("decision-1")
                    .with_sequence(1),
            )],
            Vec::new(),
            BundleMetadata::initial("0.1.0"),
        )
        .expect("bundle should be valid");

        let rendered = render_graph_bundle(&bundle);

        assert!(rendered.content.contains("[motivational]"));
        assert!(
            rendered
                .content
                .contains("because reserve power must be diverted before repair")
        );
        assert!(rendered.content.contains("decision=decision-1"));
        assert!(rendered.content.contains("step=1"));
    }
}
