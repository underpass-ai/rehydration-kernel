use serde_json::Value;

use crate::NatsConsumerError;
use crate::compatibility::context_async_service::ContextAsyncService;
use crate::compatibility::event_envelope::EventEnvelope;
use crate::compatibility::event_envelope_parser::parse_required_envelope;
use crate::compatibility::message::NatsRequestMessage;
use crate::compatibility::publisher::NatsPublicationSink;
use crate::compatibility::rehydrate_session_request_mapping::map_rehydrate_session_query;
use crate::compatibility::rehydrate_session_request_payload::RehydrateSessionRequestPayload;
use crate::compatibility::rehydrate_session_response_publication::build_rehydrate_session_response_publication;
use crate::compatibility::subject::CompatibilitySubject;
use crate::compatibility::update_context_request_mapping::map_update_context_command;
use crate::compatibility::update_context_request_payload::UpdateContextRequestPayload;
use crate::compatibility::update_context_response_publication::build_update_context_response_publication;

#[derive(Debug)]
pub struct NatsContextCompatibilityConsumer<S, P> {
    service: S,
    publisher: P,
}

impl<S, P> NatsContextCompatibilityConsumer<S, P> {
    pub fn new(service: S, publisher: P) -> Self {
        Self { service, publisher }
    }

    pub fn describe(&self) -> String {
        "nats compatibility consumer routing context.update.request and context.rehydrate.request"
            .to_string()
    }
}

impl<S, P> NatsContextCompatibilityConsumer<S, P>
where
    S: ContextAsyncService + Send + Sync,
    P: NatsPublicationSink + Send + Sync,
{
    pub async fn consume<M>(&self, subject: &str, message: &M) -> Result<(), NatsConsumerError>
    where
        M: NatsRequestMessage + Send + Sync,
    {
        let subject = CompatibilitySubject::parse(subject)?;
        let envelope = match parse_required_envelope(message.payload()) {
            Ok(envelope) => envelope,
            Err(NatsConsumerError::InvalidPayload(_))
            | Err(NatsConsumerError::InvalidEnvelope(_)) => {
                message.ack().await?;
                return Ok(());
            }
            Err(error) => return Err(error),
        };

        let publication = match subject {
            CompatibilitySubject::UpdateRequest => self.handle_update_request(&envelope).await,
            CompatibilitySubject::RehydrateRequest => {
                self.handle_rehydrate_request(&envelope).await
            }
        };

        match publication {
            Ok(publication) => {
                self.publisher.publish(publication).await?;
                message.ack().await
            }
            Err(error) => {
                message.nak().await?;
                Err(error)
            }
        }
    }

    async fn handle_update_request(
        &self,
        envelope: &EventEnvelope,
    ) -> Result<crate::compatibility::NatsPublication, NatsConsumerError> {
        let payload = parse_payload::<UpdateContextRequestPayload>(envelope)?;
        let story_id = payload.story_id.clone();
        let command = map_update_context_command(payload)?;
        let outcome = self
            .service
            .update_context(command)
            .await
            .map_err(NatsConsumerError::Application)?;

        build_update_context_response_publication(&story_id, &outcome)
    }

    async fn handle_rehydrate_request(
        &self,
        envelope: &EventEnvelope,
    ) -> Result<crate::compatibility::NatsPublication, NatsConsumerError> {
        let payload = parse_payload::<RehydrateSessionRequestPayload>(envelope)?;
        let query = map_rehydrate_session_query(payload)?;
        let result = self
            .service
            .rehydrate_session(query)
            .await
            .map_err(NatsConsumerError::Application)?;

        build_rehydrate_session_response_publication(&result)
    }
}

fn parse_payload<T>(envelope: &EventEnvelope) -> Result<T, NatsConsumerError>
where
    T: serde::de::DeserializeOwned,
{
    let payload = envelope
        .payload_object()?
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<serde_json::Map<String, Value>>();

    serde_json::from_value(Value::Object(payload))
        .map_err(|error| NatsConsumerError::InvalidRequest(error.to_string()))
}
