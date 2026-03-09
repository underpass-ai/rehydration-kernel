#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NatsPublication {
    pub subject: String,
    pub payload: Vec<u8>,
}
