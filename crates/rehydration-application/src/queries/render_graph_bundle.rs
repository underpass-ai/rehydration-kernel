use std::collections::BTreeMap;

use rehydration_domain::{RehydrationBundle, TokenEstimator};

use crate::queries::ContextRenderOptions;
use crate::queries::bundle_section_renderer::ordered_sections;
use crate::queries::bundle_truncator::{TruncationMetadata, limit_sections_by_token_budget};
use crate::queries::cl100k_estimator::Cl100kEstimator;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedSection {
    pub content: String,
    pub token_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedContext {
    pub content: String,
    pub token_count: u32,
    pub sections: Vec<RenderedSection>,
    pub truncation: Option<TruncationMetadata>,
}

pub fn render_graph_bundle(bundle: &RehydrationBundle) -> RenderedContext {
    render_graph_bundle_with_estimator(
        bundle,
        &ContextRenderOptions::default(),
        &Cl100kEstimator::new(),
    )
}

pub fn render_graph_bundle_with_options(
    bundle: &RehydrationBundle,
    options: &ContextRenderOptions,
) -> RenderedContext {
    render_graph_bundle_with_estimator(bundle, options, &Cl100kEstimator::new())
}

pub fn render_graph_bundle_with_estimator(
    bundle: &RehydrationBundle,
    options: &ContextRenderOptions,
    estimator: &dyn TokenEstimator,
) -> RenderedContext {
    let detail_by_node_id = bundle
        .node_details()
        .iter()
        .map(|detail| (detail.node_id(), detail))
        .collect::<BTreeMap<_, _>>();

    let all_sections = ordered_sections(bundle, &detail_by_node_id, options);
    let total_sections = all_sections.len() as u32;

    let (section_strings, truncation) =
        limit_sections_by_token_budget(all_sections, options, estimator, total_sections);

    let content = section_strings.join("\n\n");
    let token_count = estimator.estimate_tokens(&content);

    let sections = section_strings
        .into_iter()
        .map(|s| {
            let tc = estimator.estimate_tokens(&s);
            RenderedSection {
                content: s,
                token_count: tc,
            }
        })
        .collect();

    RenderedContext {
        content,
        token_count,
        sections,
        truncation,
    }
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
    fn render_graph_bundle_orders_root_relationships_neighbors_and_details() {
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
        assert!(rendered.sections[0].content.starts_with("Node Root"));
        assert!(rendered.sections[1].content.starts_with("Relationship"));
        assert!(rendered.sections[2].content.starts_with("Node Neighbor"));
        assert!(rendered.sections[3].content.starts_with("Detail case-123"));
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

        assert!(rendered.sections[0].content.starts_with("Node Root"));
        assert!(rendered.sections[1].content.starts_with("Node Focused"));
        assert!(rendered.sections[2].content.contains("node-2"));
    }

    #[test]
    fn render_graph_bundle_respects_token_budget_after_reordering() {
        let bundle = sample_bundle();

        let rendered = render_graph_bundle_with_options(
            &bundle,
            &ContextRenderOptions {
                focus_node_id: Some("node-2".to_string()),
                token_budget: Some(10),
            },
        );

        assert!(
            rendered.sections.len() < 7,
            "budget should truncate sections"
        );
        assert!(rendered.content.starts_with("Node Root"));
        let truncation = rendered
            .truncation
            .as_ref()
            .expect("should have truncation metadata");
        assert_eq!(truncation.budget_requested, 10);
        assert!(truncation.sections_dropped > 0);
        assert_eq!(truncation.token_estimator, "cl100k_base");
    }

    #[test]
    fn render_graph_bundle_uses_cl100k_base_estimator() {
        let bundle = RehydrationBundle::new(
            CaseId::new("case-1").expect("case id is valid"),
            Role::new("dev").expect("role is valid"),
            BundleNode::new(
                "case-1",
                "case",
                "Root",
                "",
                "ACTIVE",
                vec![],
                BTreeMap::new(),
            ),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            BundleMetadata::initial("0.1.0"),
        )
        .expect("bundle should be valid");

        let rendered = render_graph_bundle(&bundle);
        assert!(rendered.token_count > 0);
        assert!(rendered.token_count < 20);
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

    #[test]
    fn render_without_budget_has_no_truncation_metadata() {
        let bundle = sample_bundle();
        let rendered = render_graph_bundle(&bundle);
        assert!(rendered.truncation.is_none());
    }

    #[test]
    fn render_with_budget_reports_truncation_metadata() {
        let bundle = sample_bundle();
        let rendered = render_graph_bundle_with_options(
            &bundle,
            &ContextRenderOptions {
                focus_node_id: None,
                token_budget: Some(1000),
            },
        );
        let truncation = rendered
            .truncation
            .expect("budget should produce truncation");
        assert_eq!(truncation.budget_requested, 1000);
        assert_eq!(truncation.sections_dropped, 0);
        assert_eq!(truncation.token_estimator, "cl100k_base");
        assert!(truncation.budget_used <= 1000);
    }

    #[test]
    fn causal_relationships_render_before_structural() {
        let bundle = RehydrationBundle::new(
            CaseId::new("root").expect("valid"),
            Role::new("dev").expect("valid"),
            BundleNode::new(
                "root",
                "case",
                "Root",
                "",
                "ACTIVE",
                vec![],
                BTreeMap::new(),
            ),
            vec![
                BundleNode::new("a", "task", "A", "", "ACTIVE", vec![], BTreeMap::new()),
                BundleNode::new("b", "task", "B", "", "ACTIVE", vec![], BTreeMap::new()),
            ],
            vec![
                BundleRelationship::new(
                    "root",
                    "a",
                    "CONTAINS",
                    RelationExplanation::new(RelationSemanticClass::Structural),
                ),
                BundleRelationship::new(
                    "root",
                    "b",
                    "CAUSED",
                    RelationExplanation::new(RelationSemanticClass::Causal)
                        .with_rationale("failure triggered reroute"),
                ),
            ],
            Vec::new(),
            BundleMetadata::initial("0.1.0"),
        )
        .expect("valid");

        let rendered = render_graph_bundle(&bundle);

        // Causal relationship must appear before structural in rendered output
        let causal_pos = rendered
            .content
            .find("[causal]")
            .expect("causal should be present");
        let structural_pos = rendered
            .content
            .find("[structural]")
            .expect("structural should be present");
        assert!(
            causal_pos < structural_pos,
            "causal ({causal_pos}) must render before structural ({structural_pos})"
        );
    }

    #[test]
    fn section_token_counts_use_cl100k_base_not_whitespace() {
        let bundle = sample_bundle();
        let rendered = render_graph_bundle(&bundle);

        for section in &rendered.sections {
            // Whitespace count and cl100k_base count differ for most text.
            // The important invariant: token_count is computed by cl100k_base,
            // NOT by split_whitespace. For structured text like "Node Root (case):
            // Root summary", cl100k_base produces fewer tokens than words.
            let whitespace_count = section.content.split_whitespace().count() as u32;
            // cl100k_base should differ from whitespace count for most sections
            // (they're not equal for structured text). At minimum, token_count > 0.
            assert!(
                section.token_count > 0,
                "section token_count should be positive"
            );
            // The key test: the section token_count should NOT equal whitespace count
            // for sections with punctuation (which is most of ours)
            if section.content.contains('(') || section.content.contains('[') {
                assert_ne!(
                    section.token_count,
                    whitespace_count,
                    "section '{}' token_count {} should differ from whitespace count {} \
                     (proves cl100k_base, not split_whitespace)",
                    &section.content[..section.content.len().min(40)],
                    section.token_count,
                    whitespace_count
                );
            }
        }
    }
}
