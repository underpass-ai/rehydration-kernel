use rehydration_application::{GetContextResult, RenderedContext, RenderedTier};
use rehydration_domain::{RehydrationMode, ResolutionTier};
use rehydration_proto::v1beta1::{
    BundleRenderFormat, BundleSection, RenderedContext as ProtoRenderedContext,
    RenderedTier as ProtoRenderedTier,
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
        tiers: rendered
            .tiers
            .iter()
            .map(|tier| proto_rendered_tier_v1beta1(tier, scopes))
            .collect(),
        resolved_mode: proto_rehydration_mode(rendered.resolved_mode) as i32,
    }
}

fn proto_rehydration_mode(mode: RehydrationMode) -> rehydration_proto::v1beta1::RehydrationMode {
    match mode {
        RehydrationMode::Auto => rehydration_proto::v1beta1::RehydrationMode::Unspecified,
        RehydrationMode::ResumeFocused => {
            rehydration_proto::v1beta1::RehydrationMode::ResumeFocused
        }
        RehydrationMode::ReasonPreserving => {
            rehydration_proto::v1beta1::RehydrationMode::ReasonPreserving
        }
        RehydrationMode::TemporalDelta => {
            rehydration_proto::v1beta1::RehydrationMode::TemporalDelta
        }
        RehydrationMode::GlobalSummary => {
            rehydration_proto::v1beta1::RehydrationMode::GlobalSummary
        }
    }
}

fn proto_rendered_tier_v1beta1(tier: &RenderedTier, scopes: &[String]) -> ProtoRenderedTier {
    ProtoRenderedTier {
        tier: proto_resolution_tier(tier.tier) as i32,
        content: tier.content.clone(),
        token_count: tier.token_count,
        sections: tier
            .sections
            .iter()
            .enumerate()
            .map(|(index, section)| BundleSection {
                key: format!("{}_{index}", tier.tier.as_str()),
                title: format!("{} {}", tier_label(tier.tier), index + 1),
                content: section.content.clone(),
                token_count: section.token_count,
                scopes: scopes.to_vec(),
            })
            .collect(),
    }
}

fn proto_resolution_tier(tier: ResolutionTier) -> rehydration_proto::v1beta1::ResolutionTier {
    match tier {
        ResolutionTier::L0Summary => rehydration_proto::v1beta1::ResolutionTier::L0Summary,
        ResolutionTier::L1CausalSpine => rehydration_proto::v1beta1::ResolutionTier::L1CausalSpine,
        ResolutionTier::L2EvidencePack => {
            rehydration_proto::v1beta1::ResolutionTier::L2EvidencePack
        }
    }
}

fn tier_label(tier: ResolutionTier) -> &'static str {
    match tier {
        ResolutionTier::L0Summary => "Summary",
        ResolutionTier::L1CausalSpine => "Causal Spine",
        ResolutionTier::L2EvidencePack => "Evidence",
    }
}
