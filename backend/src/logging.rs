use opentelemetry::trace::TracerProvider as _;
use opentelemetry::{global, KeyValue};
use opentelemetry_otlp::SpanExporter;
use opentelemetry_sdk::trace::TracerProvider;
use opentelemetry_sdk::{runtime, Resource};
use std::env;
use tokio::task::JoinHandle;
#[cfg(feature = "loki")]
use tracing_loki::BackgroundTaskController;
use tracing_opentelemetry;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

#[cfg(all(feature = "loki", feature = "otel"))]
compile_error!("Features `loki` and `otel` cannot be enabled at the same time.");

#[cfg(feature = "loki")]
pub(crate) struct LokiLoggingSubsystem {
    tracer_provider: TracerProvider,
    controller: BackgroundTaskController,
    handle: JoinHandle<()>,
}
#[cfg(feature = "loki")]
impl LokiLoggingSubsystem {
    pub(crate) async fn shutdown(self) {
        let _ = self.tracer_provider.shutdown();
        self.controller.shutdown().await;
        self.handle.await.unwrap();
    }
}

#[cfg(feature = "otel")]
struct OtelLoggingSubsystem {
    logger_provider: LoggerProvider,
}
#[cfg(feature = "otel")]
impl OtelLoggingSubsystem {
    pub(crate) async fn shutdown(self) {
        self.logger_provider.shutdown().await;
    }
}

#[cfg(feature = "loki")]
type LoggingSubsystem = LokiLoggingSubsystem;

#[cfg(feature = "otel")]
type LoggingSubsystem = OtelLoggingSubsystem;

pub(crate) fn configure_logging() -> Result<LoggingSubsystem, anyhow::Error> {
    let resource = Resource::new(vec![KeyValue::new(
        opentelemetry_semantic_conventions::resource::SERVICE_NAME,
        env!("CARGO_CRATE_NAME"),
    )]);

    // let span_exporter = SpanExporter::builder().with_tonic().build()?;
    // Scaleway only supports HTTP (need to test if protobuf is supported, or we need to use json).
    let span_exporter = SpanExporter::builder().with_http().build()?;

    let tracer_provider = TracerProvider::builder()
        .with_resource(resource.clone())
        .with_batch_exporter(span_exporter, runtime::Tokio)
        .build();

    let tracer = tracer_provider.tracer("main");

    global::set_tracer_provider(tracer_provider.clone());

    let otel_tracing_layer = tracing_opentelemetry::layer().with_tracer(tracer);

    #[cfg(feature = "otel")]
    let (logger_provider, layer) = configure_otel(resource)?;

    #[cfg(feature = "loki")]
    let (layer, controller, handle) = configure_loki()?;

    // Add a tracing filter to filter events from crates used by opentelemetry-otlp.
    // The filter levels are set as follows:
    // - Allow `info` level and above by default.
    // - Restrict `hyper`, `tonic`, and `reqwest` to `error` level logs only.
    // This ensures events generated from these crates within the OTLP Exporter are not looped back,
    // thus preventing infinite event generation.
    // Note: This will also drop events from these crates used outside the OTLP Exporter.
    // For more details, see: https://github.com/open-telemetry/opentelemetry-rust/issues/761
    let filter = EnvFilter::new("info")
        .add_directive("hyper=error".parse()?)
        .add_directive("tonic=error".parse()?)
        .add_directive("reqwest=error".parse()?);

    let filter = filter.add_directive(format!("{}=debug", env!("CARGO_CRATE_NAME")).parse()?);

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .with(layer)
        .with(otel_tracing_layer)
        .init();

    #[cfg(feature = "otel")]
    let logging_subsystem = OtelLoggingSubsystem { logger_provider };

    #[cfg(feature = "loki")]
    let logging_subsystem = LokiLoggingSubsystem {
        tracer_provider,
        handle,
        controller,
    };

    Ok(logging_subsystem)
}

#[cfg(feature = "otel")]
fn configure_otel(
    resource: Resource,
) -> anyhow::Result<(
    LoggerProvider,
    OpenTelemetryTracingBridge<LoggerProvider, Logger>,
)> {
    use opentelemetry_appender_tracing::layer;

    let log_exporter = opentelemetry_otlp::LogExporter::builder()
        .with_tonic()
        .build()?;

    let logger_provider = opentelemetry_sdk::logs::LoggerProvider::builder()
        .with_resource(resource)
        // .with_simple_exporter(log_exporter)
        .with_batch_exporter(log_exporter, runtime::Tokio)
        .build();

    let layer = layer::OpenTelemetryTracingBridge::new(&logger_provider);

    Ok((logger_provider, layer))
}

#[cfg(feature = "loki")]
fn configure_loki() -> Result<
    (
        tracing_loki::Layer,
        BackgroundTaskController,
        JoinHandle<()>,
    ),
    anyhow::Error,
> {
    use anyhow::anyhow;
    use std::process;
    use url::Url;

    let loki_url = env::var("LOKI_URL")
        .map_err(|_| anyhow!("Invalid LOKI_URL"))
        .and_then(|s| Url::parse(s.as_str()).map_err(|_| anyhow!("Invalid LOKI_URL")))?;
    let loki_token = env::var("LOKI_TOKEN").ok();

    let mut b = tracing_loki::builder()
        .label("host", "mine")?
        .extra_field("pid", format!("{}", process::id()))?;

    if let Some(loki_token) = loki_token {
        b = b.http_header("Authorization", format!("Bearer {}", loki_token))?
        // b = b.http_header("X-Token", loki_token)?
    }

    let (layer, controller, task) = b.build_controller_url(loki_url.clone())?;

    let handle = tokio::spawn(task);

    Ok((layer, controller, handle))
}
