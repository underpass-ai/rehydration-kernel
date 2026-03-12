use std::env;

pub(crate) fn parse_bool_env(name: &str, default: bool) -> bool {
    let Ok(value) = env::var(name) else {
        return default;
    };

    parse_bool_value(&value)
}

pub(crate) fn parse_bool_value(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}

#[cfg(test)]
mod tests {
    use super::parse_bool_value;

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
