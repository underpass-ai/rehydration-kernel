#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ContextRenderOptions {
    pub focus_node_id: Option<String>,
    pub token_budget: Option<u32>,
}
