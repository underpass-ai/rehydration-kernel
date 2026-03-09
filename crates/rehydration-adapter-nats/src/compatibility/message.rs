use crate::NatsConsumerError;

#[allow(async_fn_in_trait)]
pub trait NatsRequestMessage {
    fn payload(&self) -> &[u8];

    async fn ack(&self) -> Result<(), NatsConsumerError>;

    async fn nak(&self) -> Result<(), NatsConsumerError>;
}
