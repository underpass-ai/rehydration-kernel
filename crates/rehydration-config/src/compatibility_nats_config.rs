use std::env;

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

fn parse_bool_env(name: &str, default: bool) -> bool {
    let Ok(value) = env::var(name) else {
        return default;
    };

    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}

#[cfg(test)]
mod tests {
    use super::CompatibilityNatsConfig;

    #[test]
    fn defaults_match_external_compatibility_contract() {
        let config = CompatibilityNatsConfig::from_env();

        assert_eq!(config.url, "nats://nats:4222");
        assert!(config.enabled);
    }
}
