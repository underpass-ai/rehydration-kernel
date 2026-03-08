use rehydration_application::GetContextResult;
use rehydration_domain::BundleMetadata;
use rehydration_proto::fleet_context_v1::GetContextResponse;

use crate::transport::context_service_compatibility::response_mapping::prompt_blocks::proto_prompt_blocks;

pub(crate) fn proto_get_context_response(result: &GetContextResult) -> GetContextResponse {
    GetContextResponse {
        context: result.rendered.content.clone(),
        token_count: result.rendered.token_count.min(i32::MAX as u32) as i32,
        scopes: result.scope_validation.provided_scopes.clone(),
        version: proto_context_version(result.bundle.metadata()),
        blocks: Some(proto_prompt_blocks(result)),
    }
}

fn proto_context_version(metadata: &BundleMetadata) -> String {
    if metadata.content_hash.trim().is_empty() || metadata.content_hash == "pending" {
        format!("rev-{}", metadata.revision)
    } else {
        metadata.content_hash.clone()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use rehydration_application::{GetContextResult, RenderedContext, ScopeValidation};
    use rehydration_domain::{BundleMetadata, BundleNode, CaseId, RehydrationBundle, Role};

    use super::proto_get_context_response;

    #[test]
    fn response_contains_rendered_content_blocks_and_revision_fallback() {
        let bundle = RehydrationBundle::new(
            CaseId::new("case-123").expect("case id"),
            Role::new("developer").expect("role"),
            BundleNode::new(
                "case-123",
                "story",
                "Story",
                "Summary",
                "ACTIVE",
                vec!["Story".to_string()],
                BTreeMap::new(),
            ),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            BundleMetadata::initial("0.1.0"),
        )
        .expect("bundle");
        let result = GetContextResult {
            bundle,
            rendered: RenderedContext {
                content: "Rendered body".to_string(),
                token_count: 3,
                sections: vec!["Rendered body".to_string()],
            },
            scope_validation: ScopeValidation {
                allowed: true,
                required_scopes: Vec::new(),
                provided_scopes: vec!["graph".to_string()],
                missing_scopes: Vec::new(),
                extra_scopes: Vec::new(),
                reason: String::new(),
                diagnostics: Vec::new(),
            },
            served_at: std::time::SystemTime::now(),
        };

        let response = proto_get_context_response(&result);

        assert_eq!(response.context, "Rendered body");
        assert_eq!(response.version, "rev-1");
        assert_eq!(response.blocks.expect("blocks").system, "role=developer");
    }
}
