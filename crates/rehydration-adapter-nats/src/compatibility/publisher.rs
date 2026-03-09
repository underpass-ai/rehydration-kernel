use crate::NatsConsumerError;
use crate::compatibility::NatsPublication;

#[allow(async_fn_in_trait)]
pub trait NatsPublicationSink {
    async fn publish(&self, publication: NatsPublication) -> Result<(), NatsConsumerError>;
}

impl<T> NatsPublicationSink for std::sync::Arc<T>
where
    T: NatsPublicationSink + Send + Sync + ?Sized,
{
    async fn publish(&self, publication: NatsPublication) -> Result<(), NatsConsumerError> {
        self.as_ref().publish(publication).await
    }
}
