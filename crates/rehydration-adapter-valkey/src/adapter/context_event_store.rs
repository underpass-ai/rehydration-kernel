use rehydration_ports::{
    ContextEventStore, ContextUpdatedEvent, IdempotentOutcome, PortError,
};

use crate::adapter::endpoint::ValkeyEndpoint;
use crate::adapter::io::{execute_get_command, execute_set_command};

const DEFAULT_KEY_PREFIX: &str = "rehydration:cmd";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValkeyContextEventStore {
    endpoint: ValkeyEndpoint,
}

impl ValkeyContextEventStore {
    pub fn new(runtime_state_uri: impl Into<String>) -> Result<Self, PortError> {
        let endpoint = ValkeyEndpoint::parse_with_default_key_prefix(
            runtime_state_uri.into(),
            "context event",
            DEFAULT_KEY_PREFIX,
        )?;
        Ok(Self { endpoint })
    }

    fn revision_key(&self, root_node_id: &str, role: &str) -> String {
        format!("{}:rev:{}:{}", self.endpoint.key_prefix, root_node_id, role)
    }

    fn hash_key(&self, root_node_id: &str, role: &str) -> String {
        format!(
            "{}:hash:{}:{}",
            self.endpoint.key_prefix, root_node_id, role
        )
    }

    fn idempotency_key(&self, key: &str) -> String {
        format!("{}:idem:{}", self.endpoint.key_prefix, key)
    }
}

impl ContextEventStore for ValkeyContextEventStore {
    async fn append(
        &self,
        event: ContextUpdatedEvent,
        expected_revision: u64,
    ) -> Result<u64, PortError> {
        let current = self
            .current_revision(&event.root_node_id, &event.role)
            .await?;
        if current != expected_revision {
            return Err(PortError::Conflict(format!(
                "expected revision {expected_revision}, current is {current}"
            )));
        }

        let new_revision = current + 1;
        let rev_key = self.revision_key(&event.root_node_id, &event.role);
        execute_set_command(
            &self.endpoint,
            &rev_key,
            &new_revision.to_string(),
            None,
        )
        .await?;

        let hash_key = self.hash_key(&event.root_node_id, &event.role);
        execute_set_command(&self.endpoint, &hash_key, &event.content_hash, None).await?;

        if let Some(ref idem_key) = event.idempotency_key {
            let key = self.idempotency_key(idem_key);
            let value = format!("{}:{}", new_revision, event.content_hash);
            execute_set_command(&self.endpoint, &key, &value, None).await?;
        }

        Ok(new_revision)
    }

    async fn current_revision(
        &self,
        root_node_id: &str,
        role: &str,
    ) -> Result<u64, PortError> {
        let key = self.revision_key(root_node_id, role);
        match execute_get_command(&self.endpoint, &key).await? {
            Some(value) => value.parse::<u64>().map_err(|error| {
                PortError::InvalidState(format!("invalid revision value: {error}"))
            }),
            None => Ok(0),
        }
    }

    async fn find_by_idempotency_key(
        &self,
        key: &str,
    ) -> Result<Option<IdempotentOutcome>, PortError> {
        let valkey_key = self.idempotency_key(key);
        match execute_get_command(&self.endpoint, &valkey_key).await? {
            Some(value) => {
                let (revision_str, content_hash) =
                    value.split_once(':').ok_or_else(|| {
                        PortError::InvalidState(format!(
                            "malformed idempotency value: {value}"
                        ))
                    })?;
                let revision = revision_str.parse::<u64>().map_err(|error| {
                    PortError::InvalidState(format!(
                        "invalid idempotency revision: {error}"
                    ))
                })?;
                Ok(Some(IdempotentOutcome {
                    revision,
                    content_hash: content_hash.to_string(),
                }))
            }
            None => Ok(None),
        }
    }
}
