use rehydration_domain::{RehydrationBundle, RehydrationMode, RelationSemanticClass};

/// Resolves `Auto` mode into a concrete mode based on token pressure and
/// causal density.
///
/// When mode is explicit (not Auto), it passes through unchanged.
/// When Auto, the heuristic considers:
/// 1. Token pressure (budget / nodes) — tight budgets favor ResumeFocused
/// 2. Causal density — high explanatory ratio overrides pressure to preserve rationale
pub(crate) fn resolve_mode(
    explicit_mode: RehydrationMode,
    bundle: &RehydrationBundle,
    token_budget: Option<u32>,
) -> RehydrationMode {
    match explicit_mode {
        RehydrationMode::Auto => auto_detect(bundle, token_budget),
        concrete => concrete,
    }
}

/// Tokens-per-node threshold below which we switch to ResumeFocused.
///
/// Derived from the stress benchmark: 512 tokens / 49 nodes = ~10 tokens/node
/// is deeply in the "tight" regime. Meso at 4096/21 = ~195 is comfortable.
const TOKENS_PER_NODE_THRESHOLD: u32 = 30;

/// Causal density above which we keep ReasonPreserving even under token pressure.
/// At >50% explanatory relations, the rationale metadata is worth preserving.
const CAUSAL_DENSITY_PRESERVE_THRESHOLD: f64 = 0.5;

fn auto_detect(bundle: &RehydrationBundle, token_budget: Option<u32>) -> RehydrationMode {
    let Some(budget) = token_budget else {
        return RehydrationMode::ReasonPreserving;
    };

    let total_nodes = bundle.stats().selected_nodes();
    if total_nodes == 0 {
        return RehydrationMode::ReasonPreserving;
    }

    let tokens_per_node = budget / total_nodes;
    if tokens_per_node >= TOKENS_PER_NODE_THRESHOLD {
        return RehydrationMode::ReasonPreserving;
    }

    // Budget is tight — check if the graph has enough causal content to justify
    // keeping ReasonPreserving despite the pressure.
    let causal_density = bundle_causal_density(bundle);
    if causal_density >= CAUSAL_DENSITY_PRESERVE_THRESHOLD {
        RehydrationMode::ReasonPreserving
    } else {
        RehydrationMode::ResumeFocused
    }
}

/// Fraction of relationships with explanatory semantic class (causal, motivational, evidential).
fn bundle_causal_density(bundle: &RehydrationBundle) -> f64 {
    let total = bundle.relationships().len();
    if total == 0 {
        return 0.0;
    }
    let explanatory = bundle
        .relationships()
        .iter()
        .filter(|r| {
            matches!(
                r.explanation().semantic_class(),
                RelationSemanticClass::Causal
                    | RelationSemanticClass::Motivational
                    | RelationSemanticClass::Evidential
            )
        })
        .count();
    explanatory as f64 / total as f64
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use rehydration_domain::{
        BundleMetadata, BundleNode, BundleRelationship, CaseId, RehydrationBundle,
        RehydrationMode, RelationExplanation, RelationSemanticClass, Role,
    };

    use super::resolve_mode;

    fn bundle_with_nodes(count: usize) -> RehydrationBundle {
        let root = BundleNode::new(
            "case", "case", "Root", "", "ACTIVE", vec![], BTreeMap::new(),
        );
        let neighbors: Vec<_> = (0..count.saturating_sub(1))
            .map(|i| {
                BundleNode::new(
                    format!("n{i}"), "task", format!("N{i}"), "", "ACTIVE", vec![], BTreeMap::new(),
                )
            })
            .collect();
        RehydrationBundle::new(
            CaseId::new("case").expect("valid"),
            Role::new("dev").expect("valid"),
            root,
            neighbors,
            Vec::new(),
            Vec::new(),
            BundleMetadata::initial("0.1.0"),
        )
        .expect("valid")
    }

    fn bundle_with_causal_relations(node_count: usize, causal_count: usize) -> RehydrationBundle {
        let root = BundleNode::new(
            "case", "case", "Root", "", "ACTIVE", vec![], BTreeMap::new(),
        );
        let neighbors: Vec<_> = (0..node_count.saturating_sub(1))
            .map(|i| {
                BundleNode::new(
                    format!("n{i}"), "task", format!("N{i}"), "", "ACTIVE", vec![], BTreeMap::new(),
                )
            })
            .collect();
        let mut relationships = Vec::new();
        for i in 0..node_count.saturating_sub(1) {
            let class = if i < causal_count {
                RelationSemanticClass::Causal
            } else {
                RelationSemanticClass::Structural
            };
            relationships.push(BundleRelationship::new(
                "case",
                format!("n{i}"),
                "RELATES",
                RelationExplanation::new(class),
            ));
        }
        RehydrationBundle::new(
            CaseId::new("case").expect("valid"),
            Role::new("dev").expect("valid"),
            root,
            neighbors,
            relationships,
            Vec::new(),
            BundleMetadata::initial("0.1.0"),
        )
        .expect("valid")
    }

    #[test]
    fn auto_selects_resume_focused_when_budget_tight_and_structural() {
        // 49 nodes, 0 causal relations → structural graph under pressure
        let bundle = bundle_with_nodes(49);
        let mode = resolve_mode(RehydrationMode::Auto, &bundle, Some(512));
        assert_eq!(mode, RehydrationMode::ResumeFocused);
    }

    #[test]
    fn auto_keeps_reason_preserving_when_budget_tight_but_high_causal_density() {
        // 49 nodes, 40 causal / 48 total = 83% causal density → preserve rationale
        let bundle = bundle_with_causal_relations(49, 40);
        let mode = resolve_mode(RehydrationMode::Auto, &bundle, Some(512));
        assert_eq!(mode, RehydrationMode::ReasonPreserving);
    }

    #[test]
    fn auto_selects_resume_focused_when_budget_tight_and_low_causal_density() {
        // 49 nodes, 5 causal / 48 total = 10% causal density → not worth preserving
        let bundle = bundle_with_causal_relations(49, 5);
        let mode = resolve_mode(RehydrationMode::Auto, &bundle, Some(512));
        assert_eq!(mode, RehydrationMode::ResumeFocused);
    }

    #[test]
    fn auto_selects_reason_preserving_when_budget_generous() {
        let bundle = bundle_with_nodes(21);
        let mode = resolve_mode(RehydrationMode::Auto, &bundle, Some(4096));
        assert_eq!(mode, RehydrationMode::ReasonPreserving);
    }

    #[test]
    fn auto_selects_reason_preserving_when_no_budget() {
        let bundle = bundle_with_nodes(49);
        let mode = resolve_mode(RehydrationMode::Auto, &bundle, None);
        assert_eq!(mode, RehydrationMode::ReasonPreserving);
    }

    #[test]
    fn explicit_mode_passes_through() {
        let bundle = bundle_with_nodes(49);
        let mode = resolve_mode(RehydrationMode::ResumeFocused, &bundle, Some(4096));
        assert_eq!(mode, RehydrationMode::ResumeFocused);
    }

    #[test]
    fn auto_selects_resume_focused_for_single_node_tight_budget() {
        let bundle = bundle_with_nodes(1);
        let mode = resolve_mode(RehydrationMode::Auto, &bundle, Some(10));
        assert_eq!(mode, RehydrationMode::ResumeFocused);
    }
}
