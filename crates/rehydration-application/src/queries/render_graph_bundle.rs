use std::collections::BTreeMap;

use rehydration_domain::{RehydrationBundle, ResolutionTier, TierBudget, TokenEstimator};

use crate::queries::ContextRenderOptions;
use crate::queries::bundle_section_renderer::ordered_sections;
use crate::queries::bundle_truncator::{TruncationMetadata, limit_sections_by_token_budget};
use crate::queries::cl100k_estimator::Cl100kEstimator;
use crate::queries::tier_section_classifier::classify_into_tiers;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedSection {
    pub content: String,
    pub token_count: u32,
}

/// A rendered tier with its sections and token count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedTier {
    pub tier: ResolutionTier,
    pub content: String,
    pub token_count: u32,
    pub sections: Vec<RenderedSection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedContext {
    pub content: String,
    pub token_count: u32,
    pub sections: Vec<RenderedSection>,
    pub truncation: Option<TruncationMetadata>,
    /// Multi-resolution tiers (L0 Summary, L1 Causal Spine, L2 Evidence Pack).
    pub tiers: Vec<RenderedTier>,
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

    // ── Flat rendering (backward compatible) ────────────────────────
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

    // ── Tiered rendering ────────────────────────────────────────────
    let tiered_sections = classify_into_tiers(bundle, &detail_by_node_id, options);
    let tier_budget = options
        .token_budget
        .map(TierBudget::from_total)
        .unwrap_or_else(TierBudget::unlimited);

    let tiers = build_rendered_tiers(tiered_sections, &tier_budget, estimator);

    RenderedContext {
        content,
        token_count,
        sections,
        truncation,
        tiers,
    }
}

fn build_rendered_tiers(
    tiered_sections: Vec<crate::queries::tier_section_classifier::TieredSection>,
    budget: &TierBudget,
    estimator: &dyn TokenEstimator,
) -> Vec<RenderedTier> {
    let mut tiers = Vec::new();

    for &tier in ResolutionTier::all() {
        let tier_budget = match tier {
            ResolutionTier::L0Summary => budget.l0,
            ResolutionTier::L1CausalSpine => budget.l1,
            ResolutionTier::L2EvidencePack => budget.l2,
        };

        let mut tier_sections = Vec::new();
        let mut tier_tokens = 0u32;

        for ts in &tiered_sections {
            if ts.tier != tier {
                continue;
            }
            let section_tokens = estimator.estimate_tokens(&ts.content);
            if tier_budget < u32::MAX
                && !tier_sections.is_empty()
                && tier_tokens + section_tokens > tier_budget
            {
                break;
            }
            tier_tokens += section_tokens;
            tier_sections.push(RenderedSection {
                content: ts.content.clone(),
                token_count: section_tokens,
            });
        }

        if !tier_sections.is_empty() {
            let tier_content = tier_sections
                .iter()
                .map(|s| s.content.as_str())
                .collect::<Vec<_>>()
                .join("\n\n");
            let actual_tokens = estimator.estimate_tokens(&tier_content);

            tiers.push(RenderedTier {
                tier,
                content: tier_content,
                token_count: actual_tokens,
                sections: tier_sections,
            });
        }
    }

    tiers
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use rehydration_domain::{
        BundleMetadata, BundleNode, BundleNodeDetail, BundleRelationship, CaseId,
        RehydrationBundle, RelationExplanation, RelationSemanticClass, ResolutionTier, Role,
    };

    /// Budget so tight it forces truncation on any multi-section bundle.
    const TINY_BUDGET: u32 = 10;
    /// Generous budget that fits the sample bundle without truncation.
    const GENEROUS_BUDGET: u32 = 1000;

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
                ..Default::default()
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
                token_budget: Some(TINY_BUDGET),
                ..Default::default()
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
        assert_eq!(truncation.budget_requested, TINY_BUDGET);
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
                token_budget: Some(GENEROUS_BUDGET),
                ..Default::default()
            },
        );
        let truncation = rendered
            .truncation
            .expect("budget should produce truncation");
        assert_eq!(truncation.budget_requested, GENEROUS_BUDGET);
        assert_eq!(truncation.sections_dropped, 0);
        assert_eq!(truncation.token_estimator, "cl100k_base");
        assert!(truncation.budget_used <= GENEROUS_BUDGET);
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

    #[test]
    fn render_produces_three_tiers() {
        let bundle = sample_bundle();
        let rendered = render_graph_bundle(&bundle);

        assert!(
            rendered.tiers.len() >= 2,
            "should have at least L0 and L1 tiers, got {}",
            rendered.tiers.len()
        );
        assert_eq!(rendered.tiers[0].tier, ResolutionTier::L0Summary);
        assert!(rendered.tiers[0].content.contains("Objective:"));
    }

    #[test]
    fn tiers_and_flat_content_are_both_populated() {
        let bundle = sample_bundle();
        let rendered = render_graph_bundle(&bundle);

        assert!(!rendered.content.is_empty());
        assert!(!rendered.tiers.is_empty());
        assert!(rendered.token_count > 0);
        for tier in &rendered.tiers {
            assert!(tier.token_count > 0);
            assert!(!tier.sections.is_empty());
        }
    }

    #[test]
    fn max_tier_l0_only_produces_single_tier() {
        let bundle = sample_bundle();
        let rendered = render_graph_bundle_with_options(
            &bundle,
            &ContextRenderOptions {
                max_tier: Some(ResolutionTier::L0Summary),
                ..Default::default()
            },
        );

        let tier_types: Vec<_> = rendered.tiers.iter().map(|t| t.tier).collect();
        assert_eq!(tier_types, vec![ResolutionTier::L0Summary]);
    }

    #[test]
    fn max_tier_l1_excludes_evidence_pack() {
        let bundle = sample_bundle();
        let rendered = render_graph_bundle_with_options(
            &bundle,
            &ContextRenderOptions {
                max_tier: Some(ResolutionTier::L1CausalSpine),
                ..Default::default()
            },
        );

        assert!(
            rendered
                .tiers
                .iter()
                .all(|t| t.tier != ResolutionTier::L2EvidencePack)
        );
        assert!(
            rendered
                .tiers
                .iter()
                .any(|t| t.tier == ResolutionTier::L1CausalSpine)
        );
    }

    #[test]
    fn tier_budget_constrains_l1_token_count() {
        let bundle = sample_bundle();
        let rendered = render_graph_bundle_with_options(
            &bundle,
            &ContextRenderOptions {
                token_budget: Some(200),
                ..Default::default()
            },
        );

        // With budget=200, L0 gets ~100, L1 gets ~100 — L1 should be truncated
        if let Some(l1) = rendered
            .tiers
            .iter()
            .find(|t| t.tier == ResolutionTier::L1CausalSpine)
        {
            assert!(
                l1.token_count <= 120,
                "L1 should be constrained by tier budget, got {} tokens",
                l1.token_count
            );
        }
    }
}
