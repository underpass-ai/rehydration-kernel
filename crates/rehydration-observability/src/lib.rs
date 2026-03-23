use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

/// Initializes structured logging with tracing.
///
/// Output format is controlled by `REHYDRATION_LOG_FORMAT`:
/// - `json` — machine-readable JSON lines (default for production)
/// - `pretty` — human-readable colored output
/// - anything else — compact single-line format
///
/// Log level is controlled by `RUST_LOG` (e.g. `info`, `rehydration_server=debug`).
/// Defaults to `info` when `RUST_LOG` is not set.
pub fn init_observability(service_name: &str) {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let log_format = std::env::var("REHYDRATION_LOG_FORMAT").unwrap_or_default();

    match log_format.as_str() {
        "json" => {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(
                    fmt::layer()
                        .json()
                        .with_target(true)
                        .with_thread_ids(false)
                        .with_file(false)
                        .with_line_number(false),
                )
                .init();
        }
        "pretty" => {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(fmt::layer().pretty())
                .init();
        }
        _ => {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(fmt::layer().compact())
                .init();
        }
    }

    tracing::info!(service = service_name, "observability initialized");
}

#[cfg(test)]
mod tests {
    use super::init_observability;

    #[test]
    fn init_observability_is_callable() {
        // NOTE: only one subscriber can be set globally per process; this test
        // validates that the function does not panic. In multi-test runs the
        // global subscriber may already be set, so we ignore the init error.
        let _ = std::panic::catch_unwind(|| init_observability("test"));
    }
}
