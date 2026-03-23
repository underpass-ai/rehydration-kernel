use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

/// Initializes structured logging with optional OpenTelemetry trace export.
///
/// ## Environment variables
///
/// - `RUST_LOG`: log level filter (default: `info`)
/// - `REHYDRATION_LOG_FORMAT`: `json` | `pretty` | compact (default)
/// - `OTEL_EXPORTER_OTLP_ENDPOINT`: OTLP endpoint for trace export
///   (e.g. `http://localhost:4317`). When set, traces are exported via
///   gRPC OTLP. When unset, only local logging is active.
/// - `OTEL_SERVICE_NAME`: overrides the service name in exported traces
pub fn init_observability(service_name: &str) -> Option<SdkTracerProvider> {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let log_format = std::env::var("REHYDRATION_LOG_FORMAT").unwrap_or_default();

    let otel_provider = init_otel_tracer(service_name);
    let otel_layer = otel_provider.as_ref().map(|provider| {
        let tracer = provider.tracer(service_name.to_string());
        tracing_opentelemetry::layer().with_tracer(tracer)
    });

    match log_format.as_str() {
        "json" => {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(otel_layer)
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
                .with(otel_layer)
                .with(fmt::layer().pretty())
                .init();
        }
        _ => {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(otel_layer)
                .with(fmt::layer().compact())
                .init();
        }
    }

    tracing::info!(service = service_name, "observability initialized");
    otel_provider
}

fn init_otel_tracer(service_name: &str) -> Option<SdkTracerProvider> {
    let endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok()?;
    if endpoint.trim().is_empty() {
        return None;
    }

    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .build()
        .ok()?;

    let provider = SdkTracerProvider::builder()
        .with_resource(
            opentelemetry_sdk::Resource::builder()
                .with_service_name(service_name.to_string())
                .build(),
        )
        .with_batch_exporter(exporter)
        .build();

    Some(provider)
}

/// Shuts down the OpenTelemetry trace provider, flushing pending spans.
pub fn shutdown_observability(provider: Option<SdkTracerProvider>) {
    if let Some(provider) = provider
        && let Err(error) = provider.shutdown()
    {
        tracing::warn!(%error, "opentelemetry shutdown failed");
    }
}

#[cfg(test)]
mod tests {
    use super::shutdown_observability;

    #[test]
    fn shutdown_handles_none_provider_gracefully() {
        // Verifies that shutdown with no OTel provider is a no-op
        shutdown_observability(None);
    }
}
