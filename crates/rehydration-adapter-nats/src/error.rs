use std::error::Error;
use std::fmt;

use rehydration_application::ApplicationError;

#[derive(Debug)]
pub enum NatsConsumerError {
    UnsupportedSubject(String),
    InvalidPayload(String),
    Application(ApplicationError),
}

impl fmt::Display for NatsConsumerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSubject(subject) => write!(f, "unsupported subject: {subject}"),
            Self::InvalidPayload(message) => write!(f, "invalid payload: {message}"),
            Self::Application(error) => error.fmt(f),
        }
    }
}

impl Error for NatsConsumerError {}
