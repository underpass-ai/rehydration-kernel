use crate::NatsConsumerError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompatibilitySubject {
    UpdateRequest,
    RehydrateRequest,
}

impl CompatibilitySubject {
    pub(crate) fn parse(subject: &str) -> Result<Self, NatsConsumerError> {
        match subject.trim() {
            "context.update.request" => Ok(Self::UpdateRequest),
            "context.rehydrate.request" => Ok(Self::RehydrateRequest),
            other => Err(NatsConsumerError::UnsupportedSubject(other.to_string())),
        }
    }
}
