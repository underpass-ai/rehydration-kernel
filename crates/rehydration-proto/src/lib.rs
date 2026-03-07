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
}
