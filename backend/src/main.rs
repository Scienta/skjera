mod html;
mod macros;
#[cfg(any())]
mod meta;
mod model;
mod oauth;
mod session;
#[cfg(any())]
mod skjera;
mod slack;
mod web;

use crate::model::*;
use crate::slack::SlackConnect;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use oauth2::basic::BasicClient;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry::{global, KeyValue};
use opentelemetry_appender_tracing::layer;
use opentelemetry_otlp::{LogExporter, SpanExporter};
use opentelemetry_sdk::logs::LoggerProvider;
use opentelemetry_sdk::trace::TracerProvider;
use opentelemetry_sdk::{runtime, Resource};
use reqwest::Client as ReqwestClient;
use sqlx::postgres::PgConnectOptions;
use std::env;
use std::process::exit;
use tokio::net::TcpListener;
use tokio::signal;
use tower_http::trace::TraceLayer;
use tower_sessions::cookie::SameSite::Lax;
use tower_sessions::{MemoryStore, SessionManagerLayer, SessionStore};
use tracing::{debug, info, warn};
use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    // We don't care if there is a problem here
    let env = dotenv::dotenv();
    let is_local = env.is_ok();

    let providers = configure_logging();
    if let Err(err) = providers {
        println!("{}", err);
        return;
    }
    let providers = providers.unwrap();

    warn!("skjera starting");

    let options = match std::env::var("DATABASE_URL") {
        Ok(url) => match url.parse::<PgConnectOptions>() {
            Ok(options) => options,
            Err(e) => {
                eprintln!("error: {}", e);
                exit(1)
            }
        },
        Err(_) => PgConnectOptions::default(),
    };

    info!("DATABASE_URL: {:?}", &options);

    debug!("DEBUG");

    // match run_migrations(options.clone()).await {
    //     Ok(_) => info!("migrations applies"),
    //     Err(err) => warn!("could not apply migrations: {}", err),
    // }

    let pool = sqlx::postgres::PgPool::connect_lazy_with(options);
    let assets_path = if is_local { "backend/assets" } else { "assets" }.to_string();
    let ctx = ReqwestClient::new();
    let cfg = match Config::new() {
        Ok(c) => c,
        Err(s) => {
            eprintln!("error {}", s);
            exit(1)
        }
    };
    let basic_client = oauth::build_oauth_client(
        cfg.redirect_url.clone(),
        cfg.client_id.clone(),
        cfg.client_secret.clone(),
    );

    let slack_connect = match &cfg.slack_config {
        Some(sc) => SlackConnect::new(
            sc.client_id.clone(),
            sc.client_secret.clone(),
            sc.redirect_url.clone(),
        )
        .await
        .ok(),
        None => None,
    };

    let server_impl = ServerImpl {
        pool: pool.clone(),
        assets_path,
        ctx,
        cfg,
        basic_client,
        employee_dao: EmployeeDao::new(pool),
        slack_connect,
    };

    // let tracer = tracer("my_tracer");
    //
    // tracer.in_span("doing_work", |_cx| {
    //     info!(name: "my-event-name", target: "my-system", event_id = 20, user_name = "otel", user_email = "otel@opentelemetry.io", message = "This is an example message");
    // });

    let session_store = MemoryStore::default();
    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(true)
        .with_http_only(true)
        .with_same_site(Lax);

    start_server(server_impl, session_layer, "0.0.0.0:8080").await;

    providers.0.shutdown().unwrap();
    providers.1.shutdown().unwrap();
}

fn configure_logging() -> Result<(TracerProvider, LoggerProvider), anyhow::Error> {
    let resource = Resource::new(vec![KeyValue::new(
        opentelemetry_semantic_conventions::resource::SERVICE_NAME,
        env!("CARGO_CRATE_NAME"),
    )]);

    let span_exporter = SpanExporter::builder().with_tonic().build()?;

    let tracer_provider = TracerProvider::builder()
        .with_resource(resource.clone())
        // .with_simple_exporter(span_exporter)
        .with_batch_exporter(span_exporter, runtime::Tokio)
        .build();

    let tracer = tracer_provider.tracer("main");

    global::set_tracer_provider(tracer_provider.clone());

    let otel_tracing_layer = tracing_opentelemetry::layer().with_tracer(tracer);

    let log_exporter = LogExporter::builder().with_tonic().build()?;

    let logger_provider = LoggerProvider::builder()
        .with_resource(resource)
        // .with_simple_exporter(log_exporter)
        .with_batch_exporter(log_exporter, runtime::Tokio)
        .build();

    let otel_layer = layer::OpenTelemetryTracingBridge::new(&logger_provider);

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
        .with(otel_layer)
        .with(otel_tracing_layer)
        .init();

    Ok((tracer_provider, logger_provider))
}

#[derive(Clone, Debug)]
struct ServerImpl {
    /// TODO: Figure out how to best handle the passing of the pool. Right now it is used inside
    /// EmployeeDao, but not anywhere else. I'm not sure if cloning the Pool is ok or not.
    /// Perhaps the EmployeeDao shouldn't use the pool at all and everything should just use this
    /// single reference.
    #[allow(dead_code)]
    pool: sqlx::PgPool,
    assets_path: String,
    ctx: ReqwestClient,
    cfg: Config,
    basic_client: BasicClient,
    pub employee_dao: EmployeeDao,
    pub slack_connect: Option<SlackConnect>,
}

async fn start_server<SS>(
    server_impl: ServerImpl,
    session_layer: SessionManagerLayer<SS>,
    addr: &str,
) where
    SS: SessionStore + Clone,
{
    let app = web::create_router(server_impl)
        .layer(session_layer)
        .layer(TraceLayer::new_for_http());

    // Run the server with graceful shutdown
    let listener = TcpListener::bind(addr).await.unwrap();
    info!("skjera is listening on {}", addr);
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

#[derive(Clone, Debug)]
struct Config {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_url: String,
    pub slack_config: Option<SlackConfig>,
}

impl Config {
    fn new() -> Result<Self, String> {
        let client_id =
            std::env::var("OAUTH_CLIENT_ID").map_err(|_| "OAUTH_CLIENT_ID not set".to_string())?;

        let client_secret = std::env::var("OAUTH_CLIENT_SECRET")
            .map_err(|_| "OAUTH_CLIENT_SECRET not set".to_string())?;

        let redirect_url = std::env::var("OAUTH_REDIRECT_URL")
            .map_err(|_| "OAUTH_REDIRECT_URL not set".to_string())?;

        let slack_config = match (
            env::var("SLACK_CLIENT_ID"),
            env::var("SLACK_CLIENT_SECRET"),
            env::var("SLACK_REDIRECT_URL"),
        ) {
            (Ok(client_id), Ok(client_secret), Ok(redirect_url)) => {
                Some(SlackConfig::new(client_id, client_secret, redirect_url))
            }
            _ => None,
        };

        Ok(Config {
            client_id,
            client_secret,
            redirect_url,
            slack_config,
        })
    }
}

#[derive(Clone, Debug)]
struct SlackConfig {
    client_id: String,
    client_secret: String,
    redirect_url: String,
}

impl SlackConfig {
    fn new(client_id: String, client_secret: String, redirect_url: String) -> Self {
        Self {
            client_id,
            client_secret,
            redirect_url,
        }
    }
}

#[derive(Debug)]
struct AppError(anyhow::Error);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        tracing::error!("Application error: {:#}", self.0);

        (StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong").into_response()
    }
}

// This enables using `?` on functions that return `Result<_, anyhow::Error>` to turn them into
// `Result<_, AppError>`. That way you don't need to do that manually.
impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

pub struct AuthRedirect;

impl IntoResponse for AuthRedirect {
    fn into_response(self) -> Response {
        Redirect::temporary("/").into_response()
    }
}
