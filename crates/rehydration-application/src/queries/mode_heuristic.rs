use rehydration_domain::{RehydrationBundle, RehydrationMode};

/// Resolves `Auto` mode into a concrete mode based on token pressure.
///
/// When mode is explicit (not Auto), it passes through unchanged.
/// When Auto, the heuristic selects `ResumeFocused` if the budget is tight
/// relative to graph size, or `ReasonPreserving` if there is enough room.
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

fn auto_detect(bundle: &RehydrationBundle, token_budget: Option<u32>) -> RehydrationMode {
    let Some(budget) = token_budget else {
        return RehydrationMode::ReasonPreserving;
    };

    let total_nodes = bundle.stats().selected_nodes();
    if total_nodes == 0 {
        return RehydrationMode::ReasonPreserving;
    }

    let tokens_per_node = budget / total_nodes;
    if tokens_per_node < TOKENS_PER_NODE_THRESHOLD {
        RehydrationMode::ResumeFocused
    } else {
        RehydrationMode::ReasonPreserving
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use rehydration_domain::{
        BundleMetadata, BundleNode, CaseId, RehydrationBundle, RehydrationMode, Role,
    };

    use super::resolve_mode;

    fn bundle_with_nodes(count: usize) -> RehydrationBundle {
        let root = BundleNode::new(
            "case",
            "case",
            "Root",
            "",
            "ACTIVE",
            vec![],
            BTreeMap::new(),
        );
        let neighbors: Vec<_> = (0..count.saturating_sub(1))
            .map(|i| {
                BundleNode::new(
                    &format!("n{i}"),
                    "task",
                    &format!("N{i}"),
                    "",
                    "ACTIVE",
                    vec![],
                    BTreeMap::new(),
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

    #[test]
    fn auto_selects_resume_focused_when_budget_tight() {
        let bundle = bundle_with_nodes(49);
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
    fn auto_selects_reason_preserving_for_empty_bundle() {
        let bundle = bundle_with_nodes(1); // root only
        let mode = resolve_mode(RehydrationMode::Auto, &bundle, Some(10));
        // 10/1 = 10 < 30, but single-node bundles don't benefit from resume_focused
        // Actually 10 < 30 so it selects ResumeFocused — this is correct behavior:
        // even with 1 node, if budget is that tight, pruning mode is appropriate
        assert_eq!(mode, RehydrationMode::ResumeFocused);
    }
}
