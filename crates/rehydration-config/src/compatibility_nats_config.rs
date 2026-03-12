use std::env;

use crate::env_bool::parse_bool_env;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompatibilityNatsConfig {
    pub url: String,
    pub enabled: bool,
}

impl CompatibilityNatsConfig {
    pub fn from_env() -> Self {
        Self {
            url: env::var("NATS_URL").unwrap_or_else(|_| "nats://nats:4222".to_string()),
            enabled: parse_bool_env("ENABLE_NATS", true),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::env_bool::parse_bool_value;

    use super::CompatibilityNatsConfig;

    #[test]
    fn defaults_match_external_compatibility_contract() {
        let config = CompatibilityNatsConfig::from_env();

        assert_eq!(config.url, "nats://nats:4222");
        assert!(config.enabled);
    }

    #[test]
    fn parse_bool_value_accepts_frozen_truthy_values() {
        for value in ["true", "TRUE", "1", " yes ", "on"] {
            assert!(parse_bool_value(value));
        }
    }

    #[test]
    fn parse_bool_value_treats_other_values_as_false() {
        for value in ["false", "0", "no", "off", ""] {
            assert!(!parse_bool_value(value));
        }
    }
}
