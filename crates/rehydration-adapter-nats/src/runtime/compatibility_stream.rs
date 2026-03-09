use async_nats::jetstream;
use async_nats::jetstream::consumer::{self, pull};
use async_nats::jetstream::stream::Config as StreamConfig;

use crate::runtime::NatsRuntimeError;

pub(crate) const STREAM_NAME: &str = "CONTEXT_COMPATIBILITY";
pub(crate) const UPDATE_CONSUMER_NAME: &str = "context-update-request";
pub(crate) const REHYDRATE_CONSUMER_NAME: &str = "context-rehydrate-request";
pub(crate) const UPDATE_REQUEST_SUBJECT: &str = "context.update.request";
pub(crate) const REHYDRATE_REQUEST_SUBJECT: &str = "context.rehydrate.request";
pub(crate) const UPDATE_RESPONSE_SUBJECT: &str = "context.update.response";
pub(crate) const REHYDRATE_RESPONSE_SUBJECT: &str = "context.rehydrate.response";
pub(crate) const CONTEXT_UPDATED_SUBJECT: &str = "context.events.updated";

pub(crate) async fn ensure_stream(
    jetstream: &jetstream::Context,
) -> Result<jetstream::stream::Stream, NatsRuntimeError> {
    jetstream
        .get_or_create_stream(StreamConfig {
            name: STREAM_NAME.to_string(),
            subjects: compatibility_subjects(),
            ..Default::default()
        })
        .await
        .map_err(|error| NatsRuntimeError::StreamSetup(error.to_string()))
}

pub(crate) async fn ensure_update_consumer(
    stream: &jetstream::stream::Stream,
) -> Result<jetstream::consumer::PullConsumer, NatsRuntimeError> {
    ensure_consumer(stream, UPDATE_CONSUMER_NAME, UPDATE_REQUEST_SUBJECT).await
}

pub(crate) async fn ensure_rehydrate_consumer(
    stream: &jetstream::stream::Stream,
) -> Result<jetstream::consumer::PullConsumer, NatsRuntimeError> {
    ensure_consumer(stream, REHYDRATE_CONSUMER_NAME, REHYDRATE_REQUEST_SUBJECT).await
}

fn compatibility_subjects() -> Vec<String> {
    [
        UPDATE_REQUEST_SUBJECT,
        REHYDRATE_REQUEST_SUBJECT,
        UPDATE_RESPONSE_SUBJECT,
        REHYDRATE_RESPONSE_SUBJECT,
        CONTEXT_UPDATED_SUBJECT,
    ]
    .into_iter()
    .map(ToString::to_string)
    .collect()
}

async fn ensure_consumer(
    stream: &jetstream::stream::Stream,
    name: &str,
    filter_subject: &str,
) -> Result<jetstream::consumer::PullConsumer, NatsRuntimeError> {
    stream
        .get_or_create_consumer(
            name,
            pull::Config {
                durable_name: Some(name.to_string()),
                filter_subject: filter_subject.to_string(),
                ack_policy: consumer::AckPolicy::Explicit,
                deliver_policy: consumer::DeliverPolicy::New,
                ..Default::default()
            },
        )
        .await
        .map_err(|error| NatsRuntimeError::ConsumerSetup(error.to_string()))
}
