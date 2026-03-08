use rehydration_application::GetContextResult;
use rehydration_proto::v1alpha1::{
    BundleRenderFormat, BundleSection, RenderedContext as ProtoRenderedContext,
};

pub(crate) fn proto_rendered_context_from_result(
    result: &GetContextResult,
) -> ProtoRenderedContext {
    ProtoRenderedContext {
        format: BundleRenderFormat::Structured as i32,
        content: result.rendered.content.clone(),
        token_count: result.rendered.token_count,
        sections: result
            .rendered
            .sections
            .iter()
            .enumerate()
            .map(|(index, section)| BundleSection {
                key: format!("section_{index}"),
                title: format!("Section {}", index + 1),
                content: section.clone(),
                token_count: section.split_whitespace().count() as u32,
                scopes: result.scope_validation.provided_scopes.clone(),
            })
            .collect(),
    }
}
