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
