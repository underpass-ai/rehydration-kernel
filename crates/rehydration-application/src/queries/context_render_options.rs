use rehydration_domain::{RehydrationMode, ResolutionTier};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ContextRenderOptions {
    pub focus_node_id: Option<String>,
    pub token_budget: Option<u32>,
    /// Maximum tier to render. `None` means all tiers (backward compatible).
    pub max_tier: Option<ResolutionTier>,
    /// Rehydration mode. `Auto` lets the kernel choose based on token pressure.
    pub rehydration_mode: RehydrationMode,
}
