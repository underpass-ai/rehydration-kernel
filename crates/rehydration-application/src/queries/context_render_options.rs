use rehydration_domain::ResolutionTier;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ContextRenderOptions {
    pub focus_node_id: Option<String>,
    pub token_budget: Option<u32>,
    /// Maximum tier to render. `None` means all tiers (backward compatible).
    pub max_tier: Option<ResolutionTier>,
}
