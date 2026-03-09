use rehydration_application::GetContextResult;
use rehydration_proto::fleet_context_v1::PromptBlocks;

pub(crate) fn proto_prompt_blocks(result: &GetContextResult) -> PromptBlocks {
    PromptBlocks {
        system: format!("role={}", result.bundle.role().as_str()),
        context: result.rendered.content.clone(),
        tools: render_tools_block(&result.scope_validation.provided_scopes),
    }
}

fn render_tools_block(scopes: &[String]) -> String {
    if scopes.is_empty() {
        String::new()
    } else {
        format!("active_scopes={}", scopes.join(","))
    }
}
