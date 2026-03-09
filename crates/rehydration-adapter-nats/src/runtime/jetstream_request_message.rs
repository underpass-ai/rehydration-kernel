use async_nats::jetstream;
use async_nats::jetstream::message::AckKind;

use crate::NatsConsumerError;
use crate::compatibility::NatsRequestMessage;

#[derive(Debug)]
pub(crate) struct JetStreamRequestMessage {
    message: jetstream::Message,
}

impl JetStreamRequestMessage {
    pub(crate) fn new(message: jetstream::Message) -> Self {
        Self { message }
    }
}

impl NatsRequestMessage for JetStreamRequestMessage {
    fn payload(&self) -> &[u8] {
        self.message.payload.as_ref()
    }

    async fn ack(&self) -> Result<(), NatsConsumerError> {
        self.message
            .ack()
            .await
            .map_err(|error| NatsConsumerError::MessageDisposition(error.to_string()))
    }

    async fn nak(&self) -> Result<(), NatsConsumerError> {
        self.message
            .ack_with(AckKind::Nak(None))
            .await
            .map_err(|error| NatsConsumerError::MessageDisposition(error.to_string()))
    }
}
