use rehydration_application::{GetContextResult, RenderedContext};
use rehydration_proto::v1beta1::{
    BundleRenderFormat, BundleSection, RenderedContext as ProtoRenderedContext,
};

pub(crate) fn proto_rendered_context_from_result_v1beta1(
    result: &GetContextResult,
) -> ProtoRenderedContext {
    proto_rendered_context_v1beta1(&result.rendered, &result.requested_scopes)
}

pub(crate) fn proto_rendered_context_v1beta1(
    rendered: &RenderedContext,
    scopes: &[String],
) -> ProtoRenderedContext {
    ProtoRenderedContext {
        format: BundleRenderFormat::Structured as i32,
        content: rendered.content.clone(),
        token_count: rendered.token_count,
        sections: rendered
            .sections
            .iter()
            .enumerate()
            .map(|(index, section)| BundleSection {
                key: format!("section_{index}"),
                title: format!("Section {}", index + 1),
                content: section.content.clone(),
                token_count: section.token_count,
                scopes: scopes.to_vec(),
            })
            .collect(),
    }
}
