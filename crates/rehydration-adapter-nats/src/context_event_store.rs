use async_nats::jetstream;
use async_nats::jetstream::stream;

use rehydration_domain::{ContextEventStore, ContextUpdatedEvent, IdempotentOutcome, PortError};

const STREAM_NAME: &str = "CONTEXT_EVENTS";

/// NATS JetStream implementation of the context event store.
///
/// Events are persisted as JetStream messages. Revision tracking uses the
/// stream sequence number. Idempotency is tracked via a separate KV-style
/// subject per key.
pub struct NatsContextEventStore {
    js: jetstream::Context,
    subject_prefix: String,
}

impl std::fmt::Debug for NatsContextEventStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NatsContextEventStore")
            .field("subject_prefix", &self.subject_prefix)
            .finish_non_exhaustive()
    }
}

impl NatsContextEventStore {
    pub async fn new(
        client: async_nats::Client,
        subject_prefix: impl Into<String>,
    ) -> Result<Self, PortError> {
        let js = jetstream::new(client);
        let prefix = subject_prefix.into();
        let subjects = vec![format!("{prefix}.cmd.>")];
        js.get_or_create_stream(stream::Config {
            name: STREAM_NAME.to_string(),
            subjects,
            retention: stream::RetentionPolicy::Limits,
            storage: stream::StorageType::File,
            ..Default::default()
        })
        .await
        .map_err(|error| {
            PortError::Unavailable(format!("jetstream stream setup failed: {error}"))
        })?;

        Ok(Self {
            js,
            subject_prefix: prefix,
        })
    }

    fn event_subject(&self, root_node_id: &str, role: &str) -> String {
        format!("{}.cmd.evt.{}.{}", self.subject_prefix, root_node_id, role)
    }

    fn idem_subject(&self, key: &str) -> String {
        format!("{}.cmd.idem.{}", self.subject_prefix, key)
    }

    async fn last_event_for(
        &self,
        root_node_id: &str,
        role: &str,
    ) -> Result<Option<(u64, String)>, PortError> {
        let subject = self.event_subject(root_node_id, role);
        let stream = self
            .js
            .get_stream(STREAM_NAME)
            .await
            .map_err(|error| PortError::Unavailable(format!("get stream failed: {error}")))?;

        match stream.get_last_raw_message_by_subject(&subject).await {
            Ok(msg) => {
                let payload = String::from_utf8_lossy(&msg.payload).to_string();
                let (rev_str, hash) = payload.split_once(':').unwrap_or(("0", ""));
                let revision = rev_str.parse::<u64>().unwrap_or(0);
                Ok(Some((revision, hash.to_string())))
            }
            Err(error) => {
                if matches!(
                    error.kind(),
                    async_nats::jetstream::stream::LastRawMessageErrorKind::NoMessageFound
                ) {
                    Ok(None)
                } else {
                    Err(PortError::Unavailable(format!(
                        "get last message failed: {error}"
                    )))
                }
            }
        }
    }
}

impl ContextEventStore for NatsContextEventStore {
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
        let subject = self.event_subject(&event.root_node_id, &event.role);
        let payload = format!("{new_revision}:{}", event.content_hash);

        self.js
            .publish(subject, payload.into())
            .await
            .map_err(|error| PortError::Unavailable(format!("publish event failed: {error}")))?
            .await
            .map_err(|error| PortError::Unavailable(format!("publish ack failed: {error}")))?;

        if let Some(ref idem_key) = event.idempotency_key {
            let idem_subject = self.idem_subject(idem_key);
            let idem_payload = format!("{new_revision}:{}", event.content_hash);
            let _ = self
                .js
                .publish(idem_subject, idem_payload.into())
                .await
                .map_err(|error| {
                    PortError::Unavailable(format!("publish idempotency failed: {error}"))
                })?
                .await;
        }

        Ok(new_revision)
    }

    async fn current_revision(&self, root_node_id: &str, role: &str) -> Result<u64, PortError> {
        match self.last_event_for(root_node_id, role).await? {
            Some((revision, _)) => Ok(revision),
            None => Ok(0),
        }
    }

    async fn current_content_hash(
        &self,
        root_node_id: &str,
        role: &str,
    ) -> Result<Option<String>, PortError> {
        match self.last_event_for(root_node_id, role).await? {
            Some((_, hash)) if !hash.is_empty() => Ok(Some(hash)),
            _ => Ok(None),
        }
    }

    async fn find_by_idempotency_key(
        &self,
        key: &str,
    ) -> Result<Option<IdempotentOutcome>, PortError> {
        let subject = self.idem_subject(key);
        let stream = self
            .js
            .get_stream(STREAM_NAME)
            .await
            .map_err(|error| PortError::Unavailable(format!("get stream failed: {error}")))?;

        match stream.get_last_raw_message_by_subject(&subject).await {
            Ok(msg) => {
                let payload = String::from_utf8_lossy(&msg.payload).to_string();
                let (rev_str, hash) = payload.split_once(':').unwrap_or(("0", ""));
                let revision = rev_str.parse::<u64>().unwrap_or(0);
                Ok(Some(IdempotentOutcome {
                    revision,
                    content_hash: hash.to_string(),
                }))
            }
            Err(error) => {
                if matches!(
                    error.kind(),
                    async_nats::jetstream::stream::LastRawMessageErrorKind::NoMessageFound
                ) {
                    Ok(None)
                } else {
                    Err(PortError::Unavailable(format!(
                        "idempotency lookup failed: {error}"
                    )))
                }
            }
        }
    }
}
