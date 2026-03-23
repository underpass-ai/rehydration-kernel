use std::future::Future;
use std::sync::Arc;
use std::time::SystemTime;

use crate::PortError;

/// Domain event emitted when a context update is accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextUpdatedEvent {
    pub root_node_id: String,
    pub role: String,
    pub revision: u64,
    pub content_hash: String,
    pub changes: Vec<ContextEventChange>,
    pub idempotency_key: Option<String>,
    pub requested_by: Option<String>,
    pub occurred_at: SystemTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextEventChange {
    pub operation: String,
    pub entity_kind: String,
    pub entity_id: String,
    pub payload_json: String,
}

/// Outcome previously accepted for a given idempotency key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdempotentOutcome {
    pub revision: u64,
    pub content_hash: String,
}

/// Append-only event store for context update commands.
///
/// Provides optimistic concurrency via `expected_revision` on append,
/// and deduplication via idempotency key lookup.
pub trait ContextEventStore {
    /// Append an event. Fails with `PortError::Conflict` if
    /// `expected_revision` does not match the current revision for this
    /// `(root_node_id, role)` aggregate.
    fn append(
        &self,
        event: ContextUpdatedEvent,
        expected_revision: u64,
    ) -> impl Future<Output = Result<u64, PortError>> + Send;

    /// Returns the current revision for the aggregate, or 0 if no events exist.
    fn current_revision(
        &self,
        root_node_id: &str,
        role: &str,
    ) -> impl Future<Output = Result<u64, PortError>> + Send;

    /// Checks if an event with this idempotency key was already accepted.
    fn find_by_idempotency_key(
        &self,
        key: &str,
    ) -> impl Future<Output = Result<Option<IdempotentOutcome>, PortError>> + Send;
}

impl<T> ContextEventStore for Arc<T>
where
    T: ContextEventStore + Send + Sync + ?Sized,
{
    async fn append(
        &self,
        event: ContextUpdatedEvent,
        expected_revision: u64,
    ) -> Result<u64, PortError> {
        self.as_ref().append(event, expected_revision).await
    }

    async fn current_revision(&self, root_node_id: &str, role: &str) -> Result<u64, PortError> {
        self.as_ref().current_revision(root_node_id, role).await
    }

    async fn find_by_idempotency_key(
        &self,
        key: &str,
    ) -> Result<Option<IdempotentOutcome>, PortError> {
        self.as_ref().find_by_idempotency_key(key).await
    }
}
