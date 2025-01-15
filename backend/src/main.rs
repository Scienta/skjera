mod birthday_bot;
mod html;
mod logging;
mod macros;
#[cfg(any())]
mod meta;
mod model;
mod oauth;
mod session;
#[cfg(any())]
mod skjera;
mod slack;
mod slack_client;
mod web;

use crate::birthday_bot::BirthdayBot;
use crate::model::*;
use crate::slack::SlackConnect;
use anyhow::anyhow;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use oauth2::basic::BasicClient;
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
// const GIT_BRANCH: &str = env!("GIT_BRANCH");
// const GIT_COMMIT: &str = env!("GIT_COMMIT");
// const GIT_DIRTY: &str = env!("GIT_DIRTY");
// SOURCE_TIMESTAMP doesn't work on my machine: https://gitlab.com/leonhard-llc/ops/-/issues/18
// const SOURCE_TIMESTAMP: &str = env!("SOURCE_TIMESTAMP");

// const VERSION_INFO: &str = concat!("{}{}", env!("GIT_COMMIT"), D);
const VERSION_INFO: &str = env!("VERSION_INFO");

#[tokio::main]
async fn main() {
    println!("Starting skjera. version={}", VERSION_INFO);

    // We don't care if there is a problem here
    let env = dotenv::dotenv();
    let is_local = env.is_ok();

    println!("Configuring logging");

    let logging_subsystem = logging::configure_logging();
    if let Err(err) = logging_subsystem {
        println!("error configuring logging {}", err);
        exit(1)
    }
    let logging_subsystem = logging_subsystem.unwrap();

    warn!(version = VERSION_INFO, "Starting skjera");

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
            ctx.clone(),
            sc.client_id.clone(),
            sc.client_secret.clone(),
            sc.redirect_url.clone(),
        )
        .await
        .ok(),
        None => None,
    };

    let birthday_bot = env::var("BIRTHDAY_BOT")
        .ok()
        .map(|assistant_id| BirthdayBot::new(async_openai::Client::new(), assistant_id));

    let server_impl = ServerImpl {
        pool: pool.clone(),
        assets_path,
        ctx,
        cfg,
        basic_client,
        employee_dao: EmployeeDao::new(pool),
        slack_connect,
        birthday_bot,
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

    let r = start_server(server_impl, session_layer, "0.0.0.0:8080").await;

    logging_subsystem.shutdown().await;

    if let Err(e) = r {
        println!("error: {}", e);
    } else {
        println!("Normal exit");
    }
}

#[derive(Clone)]
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
    pub birthday_bot: Option<BirthdayBot>,
}

async fn start_server<SS>(
    server_impl: ServerImpl,
    session_layer: SessionManagerLayer<SS>,
    addr: &str,
) -> anyhow::Result<()>
where
    SS: SessionStore + Clone,
{
    let app = web::create_router(server_impl)
        .layer(session_layer)
        .layer(TraceLayer::new_for_http());

    // Run the server with graceful shutdown
    let listener = TcpListener::bind(addr).await?;

    info!("skjera is listening on {}", addr);
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|e| anyhow!("server error {}", e))
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
