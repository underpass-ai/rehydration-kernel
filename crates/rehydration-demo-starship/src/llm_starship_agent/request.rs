use rehydration_proto::v1alpha1::GetContextRequest;

#[derive(Debug, Clone)]
pub struct LlmStarshipMissionRequest {
    pub root_node_id: String,
    pub root_node_kind: String,
    pub role: String,
}

impl LlmStarshipMissionRequest {
    pub fn reference_defaults(root_node_id: &str, root_node_kind: &str) -> Self {
        Self {
            root_node_id: root_node_id.to_string(),
            root_node_kind: root_node_kind.to_string(),
            role: "implementer".to_string(),
        }
    }
}

pub fn build_context_request(
    request: &LlmStarshipMissionRequest,
    step_node_id: &str,
) -> GetContextRequest {
    GetContextRequest {
        root_node_id: request.root_node_id.clone(),
        role: request.role.clone(),
        phase: 0,
        work_item_id: step_node_id.to_string(),
        token_budget: 4000,
        requested_scopes: Vec::new(),
        render_format: 0,
        include_debug_sections: true,
    }
}

#[cfg(test)]
mod tests {
    use super::{LlmStarshipMissionRequest, build_context_request};

    #[test]
    fn context_request_keeps_demo_defaults() {
        let request = LlmStarshipMissionRequest::reference_defaults("root", "mission");
        let context_request = build_context_request(&request, "node:step");

        assert_eq!(context_request.root_node_id, "root");
        assert_eq!(context_request.work_item_id, "node:step");
        assert_eq!(context_request.token_budget, 4000);
        assert!(context_request.include_debug_sections);
    }
}
