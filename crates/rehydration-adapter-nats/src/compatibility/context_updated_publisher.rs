use crate::NatsConsumerError;
use crate::compatibility::context_updated_event_publication::build_context_updated_publication;
use crate::compatibility::publisher::NatsPublicationSink;

#[derive(Debug, Clone)]
pub struct ContextUpdatedPublisher<P> {
    sink: P,
}

impl<P> ContextUpdatedPublisher<P> {
    pub fn new(sink: P) -> Self {
        Self { sink }
    }
}

impl<P> ContextUpdatedPublisher<P>
where
    P: NatsPublicationSink + Send + Sync,
{
    pub async fn publish(&self, story_id: &str, version: u64) -> Result<(), NatsConsumerError> {
        let publication = build_context_updated_publication(story_id, version)?;
        self.sink.publish(publication).await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tokio::sync::Mutex;

    use crate::NatsConsumerError;
    use crate::compatibility::{ContextUpdatedPublisher, NatsPublication, NatsPublicationSink};

    #[derive(Debug, Default)]
    struct RecordingSink {
        publications: Mutex<Vec<NatsPublication>>,
    }

    impl RecordingSink {
        async fn publications(&self) -> Vec<NatsPublication> {
            self.publications.lock().await.clone()
        }
    }

    impl NatsPublicationSink for RecordingSink {
        async fn publish(&self, publication: NatsPublication) -> Result<(), NatsConsumerError> {
            self.publications.lock().await.push(publication);
            Ok(())
        }
    }

    #[derive(Debug)]
    struct FailingSink;

    impl NatsPublicationSink for FailingSink {
        async fn publish(&self, _publication: NatsPublication) -> Result<(), NatsConsumerError> {
            Err(NatsConsumerError::Publish("sink failed".to_string()))
        }
    }

    #[tokio::test]
    async fn context_updated_publisher_emits_context_updated_publication() {
        let sink = Arc::new(RecordingSink::default());
        let publisher = ContextUpdatedPublisher::new(Arc::clone(&sink));

        publisher
            .publish("story-1", 2)
            .await
            .expect("publish should succeed");

        let publications = sink.publications().await;
        assert_eq!(publications.len(), 1);
        assert_eq!(publications[0].subject, "context.events.updated");
    }

    #[tokio::test]
    async fn context_updated_publisher_propagates_sink_failures() {
        let publisher = ContextUpdatedPublisher::new(FailingSink);
        let error = publisher
            .publish("story-1", 2)
            .await
            .expect_err("sink failure should bubble up");

        assert_eq!(error.to_string(), "publish error: sink failed");
    }
}
