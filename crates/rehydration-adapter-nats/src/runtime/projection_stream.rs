use async_nats::jetstream;
use async_nats::jetstream::consumer::{self, pull};
use async_nats::jetstream::stream::Config as StreamConfig;

use crate::runtime::NatsRuntimeError;

pub(crate) const STREAM_NAME: &str = "REHYDRATION_PROJECTION";
pub(crate) const GRAPH_CONSUMER_NAME: &str = "context-projection-graph-node-materialized";
pub(crate) const DETAIL_CONSUMER_NAME: &str = "context-projection-node-detail-materialized";
pub(crate) const GRAPH_SUBJECT: &str = "graph.node.materialized";
pub(crate) const DETAIL_SUBJECT: &str = "node.detail.materialized";

pub(crate) async fn ensure_stream(
    jetstream: &jetstream::Context,
    subject_prefix: &str,
) -> Result<jetstream::stream::Stream, NatsRuntimeError> {
    jetstream
        .get_or_create_stream(StreamConfig {
            name: stream_name(subject_prefix),
            subjects: projection_subjects(subject_prefix),
            ..Default::default()
        })
        .await
        .map_err(|error| NatsRuntimeError::StreamSetup(error.to_string()))
}

pub(crate) async fn ensure_graph_consumer(
    stream: &jetstream::stream::Stream,
    subject_prefix: &str,
) -> Result<jetstream::consumer::PullConsumer, NatsRuntimeError> {
    ensure_consumer(
        stream,
        GRAPH_CONSUMER_NAME,
        full_subject(subject_prefix, GRAPH_SUBJECT),
    )
    .await
}

pub(crate) async fn ensure_detail_consumer(
    stream: &jetstream::stream::Stream,
    subject_prefix: &str,
) -> Result<jetstream::consumer::PullConsumer, NatsRuntimeError> {
    ensure_consumer(
        stream,
        DETAIL_CONSUMER_NAME,
        full_subject(subject_prefix, DETAIL_SUBJECT),
    )
    .await
}

fn projection_subjects(subject_prefix: &str) -> Vec<String> {
    [
        full_subject(subject_prefix, GRAPH_SUBJECT),
        full_subject(subject_prefix, DETAIL_SUBJECT),
    ]
    .into_iter()
    .collect()
}

fn stream_name(subject_prefix: &str) -> String {
    if subject_prefix.is_empty() {
        STREAM_NAME.to_string()
    } else {
        format!(
            "{}_{}",
            STREAM_NAME,
            subject_prefix.replace('.', "_").to_ascii_uppercase()
        )
    }
}

fn full_subject(subject_prefix: &str, subject: &str) -> String {
    if subject_prefix.is_empty() {
        subject.to_string()
    } else {
        format!("{subject_prefix}.{subject}")
    }
}

async fn ensure_consumer(
    stream: &jetstream::stream::Stream,
    name: &str,
    filter_subject: String,
) -> Result<jetstream::consumer::PullConsumer, NatsRuntimeError> {
    stream
        .get_or_create_consumer(
            name,
            pull::Config {
                durable_name: Some(name.to_string()),
                filter_subject,
                ack_policy: consumer::AckPolicy::Explicit,
                deliver_policy: consumer::DeliverPolicy::All,
                ..Default::default()
            },
        )
        .await
        .map_err(|error| NatsRuntimeError::ConsumerSetup(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::{full_subject, projection_subjects, stream_name};

    #[test]
    fn projection_subjects_and_stream_name_follow_prefix_rules() {
        assert_eq!(
            projection_subjects("rehydration"),
            vec![
                "rehydration.graph.node.materialized".to_string(),
                "rehydration.node.detail.materialized".to_string(),
            ]
        );
        assert_eq!(
            projection_subjects(""),
            vec![
                "graph.node.materialized".to_string(),
                "node.detail.materialized".to_string(),
            ]
        );

        assert_eq!(
            stream_name("rehydration"),
            "REHYDRATION_PROJECTION_REHYDRATION"
        );
        assert_eq!(stream_name(""), "REHYDRATION_PROJECTION");
        assert_eq!(
            full_subject("rehydration", "graph.node.materialized"),
            "rehydration.graph.node.materialized"
        );
    }
}
