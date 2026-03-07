#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NatsProjectionConsumer {
    subject_prefix: String,
}

impl NatsProjectionConsumer {
    pub fn new(subject_prefix: String) -> Self {
        Self { subject_prefix }
    }

    pub fn describe(&self) -> String {
        format!(
            "nats projection consumer placeholder using {}.>",
            self.subject_prefix
        )
    }
}

#[cfg(test)]
mod tests {
    use super::NatsProjectionConsumer;

    #[test]
    fn describe_mentions_subject_prefix() {
        let consumer = NatsProjectionConsumer::new("rehydration".to_string());
        assert!(consumer.describe().contains("rehydration"));
    }
}
