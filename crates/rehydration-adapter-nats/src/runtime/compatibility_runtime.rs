use std::sync::Arc;

use async_nats::jetstream;
use tokio_stream::StreamExt;

use crate::compatibility::{
    ContextAsyncService, ContextUpdatedPublisher, NatsContextCompatibilityConsumer,
};
use crate::runtime::compatibility_stream::{
    REHYDRATE_REQUEST_SUBJECT, UPDATE_REQUEST_SUBJECT, ensure_rehydrate_consumer, ensure_stream,
    ensure_update_consumer,
};
use crate::runtime::jetstream_publication_sink::JetStreamPublicationSink;
use crate::runtime::jetstream_request_message::JetStreamRequestMessage;
use crate::runtime::runtime_error::NatsRuntimeError;

#[derive(Debug)]
pub struct NatsCompatibilityRuntime<S> {
    consumer: Arc<NatsContextCompatibilityConsumer<Arc<S>, Arc<JetStreamPublicationSink>>>,
    update_consumer: jetstream::consumer::PullConsumer,
    rehydrate_consumer: jetstream::consumer::PullConsumer,
    publication_sink: Arc<JetStreamPublicationSink>,
}

impl<S> NatsCompatibilityRuntime<S>
where
    S: ContextAsyncService + Send + Sync + 'static,
{
    pub async fn connect(url: &str, service: S) -> Result<Self, NatsRuntimeError> {
        let client = async_nats::connect(url)
            .await
            .map_err(|error| NatsRuntimeError::Connection(error.to_string()))?;
        let jetstream = jetstream::new(client);
        let stream = ensure_stream(&jetstream).await?;
        let publication_sink = Arc::new(JetStreamPublicationSink::new(jetstream));

        Ok(Self {
            consumer: Arc::new(NatsContextCompatibilityConsumer::new(
                Arc::new(service),
                Arc::clone(&publication_sink),
            )),
            update_consumer: ensure_update_consumer(&stream).await?,
            rehydrate_consumer: ensure_rehydrate_consumer(&stream).await?,
            publication_sink,
        })
    }

    pub fn describe(&self) -> String {
        "nats compatibility runtime consuming context.update.request and context.rehydrate.request via JetStream"
            .to_string()
    }

    pub fn context_updated_publisher(
        &self,
    ) -> ContextUpdatedPublisher<Arc<JetStreamPublicationSink>> {
        ContextUpdatedPublisher::new(Arc::clone(&self.publication_sink))
    }

    pub async fn run(self) -> Result<(), NatsRuntimeError> {
        let update_task = run_subject_loop(
            Arc::clone(&self.consumer),
            self.update_consumer,
            UPDATE_REQUEST_SUBJECT,
        );
        let rehydrate_task = run_subject_loop(
            Arc::clone(&self.consumer),
            self.rehydrate_consumer,
            REHYDRATE_REQUEST_SUBJECT,
        );

        tokio::try_join!(update_task, rehydrate_task)?;
        Ok(())
    }
}

async fn run_subject_loop<S>(
    consumer: Arc<NatsContextCompatibilityConsumer<Arc<S>, Arc<JetStreamPublicationSink>>>,
    pull_consumer: jetstream::consumer::PullConsumer,
    subject: &'static str,
) -> Result<(), NatsRuntimeError>
where
    S: ContextAsyncService + Send + Sync + 'static,
{
    let mut messages = pull_consumer
        .messages()
        .await
        .map_err(|error| NatsRuntimeError::Subscription(error.to_string()))?;

    while let Some(message) = messages.next().await {
        let message = message.map_err(|error| NatsRuntimeError::Message(error.to_string()))?;
        let message = JetStreamRequestMessage::new(message);
        consumer
            .consume(subject, &message)
            .await
            .map_err(NatsRuntimeError::Consumer)?;
    }

    Err(NatsRuntimeError::Subscription(format!(
        "subscription closed for {subject}"
    )))
}
