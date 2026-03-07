pub mod v1alpha1 {
    #![allow(clippy::all)]
    #![allow(missing_docs)]

    tonic::include_proto!("underpass.rehydration.kernel.v1alpha1");

    pub const FILE_DESCRIPTOR_SET: &[u8] =
        tonic::include_file_descriptor_set!("rehydration_kernel_v1alpha1_descriptor");
}

#[cfg(test)]
mod tests {
    use super::v1alpha1;

    #[test]
    fn descriptor_set_is_embedded() {
        let descriptor_set = std::hint::black_box(v1alpha1::FILE_DESCRIPTOR_SET);
        assert!(!descriptor_set.is_empty());
    }

    #[test]
    fn generated_messages_are_available() {
        let request = v1alpha1::GetContextRequest {
            case_id: "case-123".to_string(),
            role: "developer".to_string(),
            phase: v1alpha1::Phase::Build as i32,
            work_item_id: "task-7".to_string(),
            token_budget: 4096,
            requested_scopes: vec!["decisions".to_string()],
            render_format: v1alpha1::BundleRenderFormat::Structured as i32,
            include_debug_sections: false,
        };

        assert_eq!(request.case_id, "case-123");
    }

    #[test]
    fn generated_command_messages_are_available() {
        let request = v1alpha1::UpdateContextRequest {
            case_id: "case-123".to_string(),
            role: "developer".to_string(),
            work_item_id: "task-7".to_string(),
            changes: vec![v1alpha1::ContextChange {
                operation: v1alpha1::ContextChangeOperation::Update as i32,
                entity_kind: "decision".to_string(),
                entity_id: "decision-9".to_string(),
                payload_json: "{\"status\":\"accepted\"}".to_string(),
                reason: "agent refined decision".to_string(),
                scopes: vec!["decisions".to_string()],
            }],
            metadata: Some(v1alpha1::CommandMetadata {
                idempotency_key: "cmd-123".to_string(),
                correlation_id: "corr-123".to_string(),
                causation_id: "cause-123".to_string(),
                requested_by: "agent-executor".to_string(),
                requested_at: None,
            }),
            precondition: Some(v1alpha1::RevisionPrecondition {
                expected_revision: 4,
                expected_content_hash: "abc123".to_string(),
            }),
            persist_snapshot: true,
        };

        assert_eq!(request.changes.len(), 1);
    }
}
