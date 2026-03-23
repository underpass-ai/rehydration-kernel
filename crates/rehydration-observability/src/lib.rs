pub mod metrics;

use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

pub use metrics::KernelMetrics;

/// Resources returned by `init_observability` for lifecycle management.
pub struct ObservabilityGuard {
    trace_provider: Option<SdkTracerProvider>,
    meter_provider: Option<SdkMeterProvider>,
    pub metrics: KernelMetrics,
}

/// Initializes structured logging with optional OpenTelemetry trace and metric export.
///
/// ## Environment variables
///
/// - `RUST_LOG`: log level filter (default: `info`)
/// - `REHYDRATION_LOG_FORMAT`: `json` | `pretty` | compact (default)
/// - `OTEL_EXPORTER_OTLP_ENDPOINT`: OTLP endpoint for trace and metric export
///   (e.g. `http://localhost:4317`). When set, traces and metrics are exported
///   via gRPC OTLP. When unset, only local logging is active.
pub fn init_observability(service_name: &str) -> ObservabilityGuard {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let log_format = std::env::var("REHYDRATION_LOG_FORMAT").unwrap_or_default();

    let trace_provider = init_otel_tracer(service_name);
    let otel_layer = trace_provider.as_ref().map(|provider| {
        let tracer = provider.tracer(service_name.to_string());
        tracing_opentelemetry::layer().with_tracer(tracer)
    });

    let meter_provider = metrics::init_otel_metrics(service_name);
    if let Some(ref provider) = meter_provider {
        opentelemetry::global::set_meter_provider(provider.clone());
    }
    let meter = opentelemetry::global::meter("rehydration-kernel");
    let kernel_metrics = KernelMetrics::new(&meter);

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

    ObservabilityGuard {
        trace_provider,
        meter_provider,
        metrics: kernel_metrics,
    }
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

/// Shuts down the OpenTelemetry providers, flushing pending data.
pub fn shutdown_observability(guard: ObservabilityGuard) {
    if let Some(provider) = guard.trace_provider
        && let Err(error) = provider.shutdown()
    {
        tracing::warn!(%error, "opentelemetry trace shutdown failed");
    }
    if let Some(provider) = guard.meter_provider
        && let Err(error) = provider.shutdown()
    {
        tracing::warn!(%error, "opentelemetry metrics shutdown failed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kernel_metrics_instruments_are_constructible() {
        let meter = opentelemetry::global::meter("test");
        let metrics = KernelMetrics::new(&meter);
        // Verify instruments exist and can record without panic
        metrics.rpc_duration.record(0.1, &[]);
        metrics.bundle_nodes.record(5, &[]);
        metrics.bundle_relationships.record(3, &[]);
        metrics.bundle_details.record(2, &[]);
        metrics.rendered_tokens.record(100, &[]);
        metrics.truncation_total.add(1, &[]);
        metrics.projection_lag.record(0.05, &[]);
    }
}
