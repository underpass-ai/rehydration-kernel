use std::time::{SystemTime, UNIX_EPOCH};

const DEBUG_FLAG_ENV: &str = "AGENTIC_E2E_DEBUG";

pub(crate) fn debug_log(message: impl AsRef<str>) {
    if !debug_enabled() {
        return;
    }

    eprintln!("{}", format_log_line(message.as_ref()));
}

pub(crate) fn debug_log_value(label: &str, value: impl std::fmt::Display) {
    debug_log(format!("{label}: {value}"));
}

fn debug_enabled() -> bool {
    std::env::var(DEBUG_FLAG_ENV)
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false)
}

fn format_log_line(message: &str) -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    format!("[agentic-e2e][{millis}] {message}")
}
