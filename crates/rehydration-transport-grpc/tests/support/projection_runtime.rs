use std::error::Error;
use std::sync::Arc;

use async_nats::Subscriber;
use rehydration_adapter_nats::NatsProjectionConsumer;
use rehydration_application::{ProjectionApplicationService, RoutingProjectionWriter};
use rehydration_domain::ProjectionWriter;
use rehydration_testkit::{InMemoryProcessedEventStore, InMemoryProjectionCheckpointStore};
use tokio::task::JoinHandle;
use tokio_stream::StreamExt;

use crate::agentic_support::agentic_debug::{debug_log, debug_log_value};
use crate::agentic_support::nats_container::connect_with_retry;

pub(crate) struct RunningProjectionRuntime {
    task: JoinHandle<Result<(), Box<dyn Error + Send + Sync>>>,
}

impl RunningProjectionRuntime {
    pub(crate) async fn start<G, D>(
        nats_url: &str,
        subject_prefix: &str,
        graph_writer: G,
        detail_writer: D,
    ) -> Result<Self, Box<dyn Error + Send + Sync>>
    where
        G: ProjectionWriter + Send + Sync + 'static,
        D: ProjectionWriter + Send + Sync + 'static,
    {
        debug_log_value("projection runtime nats_url", nats_url);
        let client = connect_with_retry(nats_url).await?;
        let graph_subscription = client
            .subscribe(prefixed_subject(subject_prefix, "graph.node.materialized"))
            .await?;
        let detail_subscription = client
            .subscribe(prefixed_subject(subject_prefix, "node.detail.materialized"))
            .await?;

        let consumer = NatsProjectionConsumer::new(subject_prefix.to_string());
        let handler = Arc::new(ProjectionApplicationService::new(
            RoutingProjectionWriter::new(graph_writer, detail_writer),
            InMemoryProcessedEventStore::default(),
            InMemoryProjectionCheckpointStore::default(),
        ));

        let task = tokio::spawn(async move {
            debug_log("projection runtime loops started");
            let graph_loop = run_subscription_loop(
                consumer.clone(),
                Arc::clone(&handler),
                graph_subscription,
                "graph.node.materialized",
            );
            let detail_loop = run_subscription_loop(
                consumer,
                handler,
                detail_subscription,
                "node.detail.materialized",
            );

            tokio::try_join!(graph_loop, detail_loop)?;
            Ok(())
        });

        Ok(Self { task })
    }

    pub(crate) async fn shutdown(self) -> Result<(), Box<dyn Error + Send + Sync>> {
        debug_log("projection runtime shutdown requested");
        self.task.abort();
        match self.task.await {
            Ok(result) => result,
            Err(join_error) if join_error.is_cancelled() => Ok(()),
            Err(join_error) => Err(Box::new(join_error)),
        }
    }
}

async fn run_subscription_loop<H>(
    consumer: NatsProjectionConsumer,
    handler: Arc<H>,
    mut subscription: Subscriber,
    subject: &'static str,
) -> Result<(), Box<dyn Error + Send + Sync>>
where
    H: rehydration_application::ProjectionEventHandler + Send + Sync + 'static,
{
    while let Some(message) = subscription.next().await {
        debug_log_value("projection runtime subject", subject);
        consumer
            .consume(handler.as_ref(), subject, message.payload.as_ref())
            .await?;
    }

    Err(format!("subscription closed for {subject}").into())
}

fn prefixed_subject(subject_prefix: &str, subject: &str) -> String {
    if subject_prefix.is_empty() {
        subject.to_string()
    } else {
        format!("{subject_prefix}.{subject}")
    }
}
