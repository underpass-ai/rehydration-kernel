use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
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
    pub persist_snapshot: bool,
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
    pub snapshot_persisted: bool,
    pub snapshot_id: Option<String>,
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
                snapshot_persisted: command.persist_snapshot,
                snapshot_id: None,
            });
        }

        // Load current revision
        let current_revision = self
            .event_store
            .current_revision(case_id.as_str(), role.as_str())
            .await?;

        // Validate preconditions
        let expected_revision = command.expected_revision.unwrap_or(current_revision);
        if expected_revision != current_revision {
            return Err(ApplicationError::Ports(
                rehydration_domain::PortError::Conflict(format!(
                    "expected revision {expected_revision}, current is {current_revision}"
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
            snapshot_persisted: command.persist_snapshot,
            snapshot_id: None,
        })
    }
}

fn compute_content_hash(changes: &[UpdateContextChange]) -> String {
    let mut hasher = DefaultHasher::new();
    for change in changes {
        change.operation.hash(&mut hasher);
        change.entity_kind.hash(&mut hasher);
        change.entity_id.hash(&mut hasher);
        change.payload_json.hash(&mut hasher);
    }
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rehydration_testkit::InMemoryContextEventStore;

    use super::*;

    fn event_store() -> Arc<InMemoryContextEventStore> {
        Arc::new(InMemoryContextEventStore::new())
    }

    fn use_case(store: Arc<InMemoryContextEventStore>) -> UpdateContextUseCase<InMemoryContextEventStore> {
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
                persist_snapshot: false,
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
                persist_snapshot: false,
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
                persist_snapshot: false,
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
                persist_snapshot: false,
            })
            .await
            .expect("second call should return cached outcome");

        assert_eq!(first.accepted_version.revision, second.accepted_version.revision);
        assert_eq!(first.accepted_version.content_hash, second.accepted_version.content_hash);
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
                persist_snapshot: false,
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
                persist_snapshot: false,
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
                persist_snapshot: false,
            })
            .await
            .expect("second should succeed");

        assert_eq!(first.accepted_version.revision, 1);
        assert_eq!(second.accepted_version.revision, 2);
    }
}
