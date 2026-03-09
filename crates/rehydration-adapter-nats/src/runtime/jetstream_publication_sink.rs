use async_nats::jetstream;

use crate::NatsConsumerError;
use crate::compatibility::{NatsPublication, NatsPublicationSink};

#[derive(Debug, Clone)]
pub struct JetStreamPublicationSink {
    jetstream: jetstream::Context,
}

impl JetStreamPublicationSink {
    pub fn new(jetstream: jetstream::Context) -> Self {
        Self { jetstream }
    }
}

impl NatsPublicationSink for JetStreamPublicationSink {
    async fn publish(&self, publication: NatsPublication) -> Result<(), NatsConsumerError> {
        self.jetstream
            .publish(publication.subject, publication.payload.into())
            .await
            .map_err(|error| NatsConsumerError::Publish(error.to_string()))?
            .await
            .map_err(|error| NatsConsumerError::Publish(error.to_string()))?;

        Ok(())
    }
}
