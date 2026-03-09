use rehydration_application::{
    ApplicationError, RehydrateSessionQuery, RehydrateSessionResult, UpdateContextCommand,
    UpdateContextOutcome,
};

#[allow(async_fn_in_trait)]
pub trait ContextAsyncService {
    async fn update_context(
        &self,
        command: UpdateContextCommand,
    ) -> Result<UpdateContextOutcome, ApplicationError>;

    async fn rehydrate_session(
        &self,
        query: RehydrateSessionQuery,
    ) -> Result<RehydrateSessionResult, ApplicationError>;
}

impl<T> ContextAsyncService for std::sync::Arc<T>
where
    T: ContextAsyncService + Send + Sync + ?Sized,
{
    async fn update_context(
        &self,
        command: UpdateContextCommand,
    ) -> Result<UpdateContextOutcome, ApplicationError> {
        self.as_ref().update_context(command).await
    }

    async fn rehydrate_session(
        &self,
        query: RehydrateSessionQuery,
    ) -> Result<RehydrateSessionResult, ApplicationError> {
        self.as_ref().rehydrate_session(query).await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rehydration_application::{
        AcceptedVersion, ApplicationError, RehydrateSessionQuery, RehydrateSessionResult,
        UpdateContextCommand, UpdateContextOutcome,
    };
    use rehydration_domain::BundleMetadata;

    use super::ContextAsyncService;

    struct StubService;

    impl ContextAsyncService for StubService {
        async fn update_context(
            &self,
            _command: UpdateContextCommand,
        ) -> Result<UpdateContextOutcome, ApplicationError> {
            Ok(UpdateContextOutcome {
                accepted_version: AcceptedVersion {
                    revision: 1,
                    content_hash: "hash-1".to_string(),
                    generator_version: "0.1.0".to_string(),
                },
                warnings: Vec::new(),
                snapshot_persisted: false,
                snapshot_id: None,
            })
        }

        async fn rehydrate_session(
            &self,
            query: RehydrateSessionQuery,
        ) -> Result<RehydrateSessionResult, ApplicationError> {
            Ok(RehydrateSessionResult {
                root_node_id: query.root_node_id,
                bundles: Vec::new(),
                timeline_events: query.timeline_window,
                version: BundleMetadata::initial("0.1.0"),
                snapshot_persisted: false,
                snapshot_id: None,
                generated_at: std::time::SystemTime::UNIX_EPOCH,
            })
        }
    }

    #[tokio::test]
    async fn arc_context_async_service_delegates_calls() {
        let service = Arc::new(StubService);

        let update = service
            .update_context(UpdateContextCommand {
                root_node_id: "story-1".to_string(),
                role: "developer".to_string(),
                work_item_id: "task-1".to_string(),
                changes: Vec::new(),
                expected_revision: None,
                expected_content_hash: None,
                idempotency_key: None,
                requested_by: None,
                persist_snapshot: false,
            })
            .await
            .expect("update should succeed");
        assert_eq!(update.accepted_version.revision, 1);

        let rehydrate = service
            .rehydrate_session(RehydrateSessionQuery {
                root_node_id: "case-1".to_string(),
                roles: vec!["developer".to_string()],
                persist_snapshot: false,
                timeline_window: 50,
                snapshot_ttl_seconds: 3600,
            })
            .await
            .expect("rehydrate should succeed");
        assert_eq!(rehydrate.root_node_id, "case-1");
    }
}
