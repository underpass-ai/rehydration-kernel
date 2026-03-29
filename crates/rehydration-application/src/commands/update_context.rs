use std::sync::Arc;
use std::time::SystemTime;

use rehydration_domain::{
    CaseId, ContextEventChange, ContextEventStore, ContextUpdatedEvent, Role,
};

use crate::ApplicationError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateContextChange {
    pub operation: String,
    pub entity_kind: String,
    pub entity_id: String,
    pub payload_json: String,
    pub reason: String,
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateContextCommand {
    pub root_node_id: String,
    pub role: String,
    pub work_item_id: String,
    pub changes: Vec<UpdateContextChange>,
    pub expected_revision: Option<u64>,
    pub expected_content_hash: Option<String>,
    pub idempotency_key: Option<String>,
    pub requested_by: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedVersion {
    pub revision: u64,
    pub content_hash: String,
    pub generator_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateContextOutcome {
    pub accepted_version: AcceptedVersion,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub struct UpdateContextUseCase<E> {
    event_store: Arc<E>,
    generator_version: &'static str,
}

impl<E> UpdateContextUseCase<E>
where
    E: ContextEventStore + Send + Sync,
{
    pub fn new(event_store: Arc<E>, generator_version: &'static str) -> Self {
        Self {
            event_store,
            generator_version,
        }
    }

    pub async fn execute(
        &self,
        command: UpdateContextCommand,
    ) -> Result<UpdateContextOutcome, ApplicationError> {
        let case_id = CaseId::new(&command.root_node_id)?;
        let role = Role::new(&command.role)?;

        // Idempotency check
        if let Some(ref key) = command.idempotency_key
            && let Some(outcome) = self.event_store.find_by_idempotency_key(key).await?
        {
            return Ok(UpdateContextOutcome {
                accepted_version: AcceptedVersion {
                    revision: outcome.revision,
                    content_hash: outcome.content_hash,
                    generator_version: self.generator_version.to_string(),
                },
                warnings: vec![],
            });
        }

        // Load current revision
        let current_revision = self
            .event_store
            .current_revision(case_id.as_str(), role.as_str())
            .await?;

        // Validate revision precondition
        let expected_revision = command.expected_revision.unwrap_or(current_revision);
        if expected_revision != current_revision {
            return Err(ApplicationError::Ports(
                rehydration_domain::PortError::Conflict(format!(
                    "expected revision {expected_revision}, current is {current_revision}"
                )),
            ));
        }

        // Validate content hash precondition
        if let Some(ref expected_hash) = command.expected_content_hash
            && let Some(ref current_hash) = self
                .event_store
                .current_content_hash(case_id.as_str(), role.as_str())
                .await?
            && expected_hash != current_hash
        {
            return Err(ApplicationError::Ports(
                rehydration_domain::PortError::Conflict(format!(
                    "expected content hash '{expected_hash}', current is '{current_hash}'"
                )),
            ));
        }

        // Compute content hash from actual change payloads
        let content_hash = compute_content_hash(&command.changes);

        let mut warnings = Vec::new();
        if command.changes.is_empty() {
            warnings.push("no changes supplied; update was accepted as a no-op".to_string());
        }

        // Build domain event
        let event = ContextUpdatedEvent {
            root_node_id: case_id.as_str().to_string(),
            role: role.as_str().to_string(),
            revision: current_revision + 1,
            content_hash: content_hash.clone(),
            changes: command
                .changes
                .iter()
                .map(|c| ContextEventChange {
                    operation: c.operation.clone(),
                    entity_kind: c.entity_kind.clone(),
                    entity_id: c.entity_id.clone(),
                    payload_json: c.payload_json.clone(),
                })
                .collect(),
            idempotency_key: command.idempotency_key.clone(),
            requested_by: command.requested_by.clone(),
            occurred_at: SystemTime::now(),
        };

        // Append with optimistic concurrency
        let new_revision = self.event_store.append(event, current_revision).await?;

        Ok(UpdateContextOutcome {
            accepted_version: AcceptedVersion {
                revision: new_revision,
                content_hash,
                generator_version: self.generator_version.to_string(),
            },
            warnings,
        })
    }
}

/// Deterministic SHA-256 hash of context changes for optimistic concurrency.
/// Stable across process restarts and machines.
fn compute_content_hash(changes: &[UpdateContextChange]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    for change in changes {
        hasher.update(change.operation.as_bytes());
        hasher.update(change.entity_kind.as_bytes());
        hasher.update(change.entity_id.as_bytes());
        hasher.update(change.payload_json.as_bytes());
    }
    format!("{:064x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rehydration_testkit::InMemoryContextEventStore;

    use super::*;

    fn event_store() -> Arc<InMemoryContextEventStore> {
        Arc::new(InMemoryContextEventStore::new())
    }

    fn use_case(
        store: Arc<InMemoryContextEventStore>,
    ) -> UpdateContextUseCase<InMemoryContextEventStore> {
        UpdateContextUseCase::new(store, "0.1.0")
    }

    fn sample_change() -> UpdateContextChange {
        UpdateContextChange {
            operation: "UPDATE".to_string(),
            entity_kind: "node_detail".to_string(),
            entity_id: "node-1".to_string(),
            payload_json: r#"{"status":"ACTIVE"}"#.to_string(),
            reason: "test".to_string(),
            scopes: vec!["graph".to_string()],
        }
    }

    #[tokio::test]
    async fn execute_appends_event_and_returns_revision_one() {
        let store = event_store();
        let uc = use_case(Arc::clone(&store));

        let outcome = uc
            .execute(UpdateContextCommand {
                root_node_id: "node-1".to_string(),
                role: "developer".to_string(),
                work_item_id: "task-1".to_string(),
                changes: vec![sample_change()],
                expected_revision: None,
                expected_content_hash: None,
                idempotency_key: None,
                requested_by: None,
            })
            .await
            .expect("should succeed");

        assert_eq!(outcome.accepted_version.revision, 1);
        assert!(!outcome.accepted_version.content_hash.is_empty());
        assert!(outcome.warnings.is_empty());
    }

    #[tokio::test]
    async fn execute_rejects_wrong_expected_revision() {
        let store = event_store();
        let uc = use_case(Arc::clone(&store));

        let err = uc
            .execute(UpdateContextCommand {
                root_node_id: "node-1".to_string(),
                role: "developer".to_string(),
                work_item_id: "task-1".to_string(),
                changes: vec![sample_change()],
                expected_revision: Some(99),
                expected_content_hash: None,
                idempotency_key: None,
                requested_by: None,
            })
            .await;

        assert!(err.is_err());
        let msg = err.expect_err("should fail").to_string();
        assert!(msg.contains("expected revision 99"));
    }

    #[tokio::test]
    async fn execute_returns_cached_outcome_for_duplicate_idempotency_key() {
        let store = event_store();
        let uc = use_case(Arc::clone(&store));

        let first = uc
            .execute(UpdateContextCommand {
                root_node_id: "node-1".to_string(),
                role: "developer".to_string(),
                work_item_id: "task-1".to_string(),
                changes: vec![sample_change()],
                expected_revision: None,
                expected_content_hash: None,
                idempotency_key: Some("idem-1".to_string()),
                requested_by: None,
            })
            .await
            .expect("first call should succeed");

        let second = uc
            .execute(UpdateContextCommand {
                root_node_id: "node-1".to_string(),
                role: "developer".to_string(),
                work_item_id: "task-1".to_string(),
                changes: vec![sample_change()],
                expected_revision: None,
                expected_content_hash: None,
                idempotency_key: Some("idem-1".to_string()),
                requested_by: None,
            })
            .await
            .expect("second call should return cached outcome");

        assert_eq!(
            first.accepted_version.revision,
            second.accepted_version.revision
        );
        assert_eq!(
            first.accepted_version.content_hash,
            second.accepted_version.content_hash
        );
    }

    #[tokio::test]
    async fn execute_warns_on_empty_changes() {
        let store = event_store();
        let uc = use_case(Arc::clone(&store));

        let outcome = uc
            .execute(UpdateContextCommand {
                root_node_id: "node-1".to_string(),
                role: "developer".to_string(),
                work_item_id: "task-1".to_string(),
                changes: vec![],
                expected_revision: None,
                expected_content_hash: None,
                idempotency_key: None,
                requested_by: None,
            })
            .await
            .expect("empty changes should succeed with warning");

        assert_eq!(outcome.warnings.len(), 1);
        assert!(outcome.warnings[0].contains("no changes supplied"));
    }

    #[tokio::test]
    async fn execute_increments_revision_on_sequential_calls() {
        let store = event_store();
        let uc = use_case(Arc::clone(&store));

        let first = uc
            .execute(UpdateContextCommand {
                root_node_id: "node-1".to_string(),
                role: "developer".to_string(),
                work_item_id: "task-1".to_string(),
                changes: vec![sample_change()],
                expected_revision: None,
                expected_content_hash: None,
                idempotency_key: None,
                requested_by: None,
            })
            .await
            .expect("first should succeed");

        let second = uc
            .execute(UpdateContextCommand {
                root_node_id: "node-1".to_string(),
                role: "developer".to_string(),
                work_item_id: "task-2".to_string(),
                changes: vec![sample_change()],
                expected_revision: Some(1),
                expected_content_hash: None,
                idempotency_key: None,
                requested_by: None,
            })
            .await
            .expect("second should succeed");

        assert_eq!(first.accepted_version.revision, 1);
        assert_eq!(second.accepted_version.revision, 2);
    }

    #[tokio::test]
    async fn execute_rejects_wrong_content_hash_precondition() {
        let store = event_store();
        let uc = use_case(Arc::clone(&store));

        // First command establishes a hash
        let first = uc
            .execute(UpdateContextCommand {
                root_node_id: "node-1".to_string(),
                role: "developer".to_string(),
                work_item_id: "task-1".to_string(),
                changes: vec![sample_change()],
                expected_revision: None,
                expected_content_hash: None,
                idempotency_key: None,
                requested_by: None,
            })
            .await
            .expect("first should succeed");

        // Second command with wrong expected_content_hash must fail
        let err = uc
            .execute(UpdateContextCommand {
                root_node_id: "node-1".to_string(),
                role: "developer".to_string(),
                work_item_id: "task-2".to_string(),
                changes: vec![sample_change()],
                expected_revision: Some(1),
                expected_content_hash: Some("wrong-hash".to_string()),
                idempotency_key: None,
                requested_by: None,
            })
            .await;

        assert!(err.is_err());
        let msg = err.expect_err("should fail").to_string();
        assert!(msg.contains("expected content hash"));

        // Same command with correct hash succeeds
        let ok = uc
            .execute(UpdateContextCommand {
                root_node_id: "node-1".to_string(),
                role: "developer".to_string(),
                work_item_id: "task-2".to_string(),
                changes: vec![sample_change()],
                expected_revision: Some(1),
                expected_content_hash: Some(first.accepted_version.content_hash.clone()),
                idempotency_key: None,
                requested_by: None,
            })
            .await
            .expect("correct hash should succeed");

        assert_eq!(ok.accepted_version.revision, 2);
    }
}
