use std::error::Error;
use std::fmt;

use crate::NatsConsumerError;

#[derive(Debug)]
pub enum NatsRuntimeError {
    Connection(String),
    StreamSetup(String),
    ConsumerSetup(String),
    Subscription(String),
    Message(String),
    Consumer(NatsConsumerError),
}

impl fmt::Display for NatsRuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Connection(message) => write!(f, "nats connection error: {message}"),
            Self::StreamSetup(message) => write!(f, "nats stream setup error: {message}"),
            Self::ConsumerSetup(message) => write!(f, "nats consumer setup error: {message}"),
            Self::Subscription(message) => write!(f, "nats subscription error: {message}"),
            Self::Message(message) => write!(f, "nats message error: {message}"),
            Self::Consumer(error) => error.fmt(f),
        }
    }
}

impl Error for NatsRuntimeError {}
