mod html;
mod macros;
mod meta;
mod model;
mod skjera;

use crate::model::*;
use anyhow::Context;
use async_session::{MemoryStore, Session, SessionStore};
use async_trait::async_trait;
use axum::extract::{FromRef, FromRequestParts};
use axum::extract::{Query, State};
use axum::http::header::SET_COOKIE;
use axum::http::request::Parts;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{RequestPartsExt, Router};
use axum_extra::typed_header::TypedHeaderRejectionReason;
use axum_extra::TypedHeader;
use oauth2::reqwest::async_http_client;
use oauth2::{
    basic::BasicClient, AuthUrl, AuthorizationCode, ClientId, ClientSecret, RedirectUrl,
    TokenResponse, TokenUrl,
};
use opentelemetry::global::tracer;
use opentelemetry::trace::Tracer;
use opentelemetry::{global, KeyValue};
use opentelemetry_appender_tracing::layer;
use opentelemetry_otlp::{LogExporter, SpanExporter};
use opentelemetry_sdk::logs::LoggerProvider;
use opentelemetry_sdk::trace::TracerProvider;
use opentelemetry_sdk::{runtime, trace as sdk_trace, Resource};
use reqwest::Client as ReqwestClient;
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgConnectOptions;
use std::path::Path;
use std::process::exit;
use tokio::net::TcpListener;
use tokio::signal;
use tower_http::{services::ServeDir, trace::TraceLayer};
use tracing::{debug, info, span, warn, Level};
use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

static COOKIE_NAME: &str = "SESSION";

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
    let basic_client = build_oauth_client(
        cfg.redirect_url.clone(),
        cfg.client_id.clone(),
        cfg.client_secret.clone(),
    );
    let server_impl = ServerImpl {
        pool: pool.clone(),
        assets_path,
        ctx,
        cfg,
        basic_client,
        employee_dao: EmployeeDao::new(pool),
        store: MemoryStore::new(),
    };

    // let tracer = tracer("my_tracer");
    //
    // tracer.in_span("doing_work", |_cx| {
    //     info!(name: "my-event-name", target: "my-system", event_id = 20, user_name = "otel", user_email = "otel@opentelemetry.io", message = "This is an example message");
    // });

    start_server(server_impl, "0.0.0.0:8080").await;

    providers.0.shutdown().unwrap();
    providers.1.shutdown().unwrap();
}

fn configure_logging() -> Result<(TracerProvider, LoggerProvider), anyhow::Error> {
    let resource = Resource::new(vec![KeyValue::new(
        opentelemetry_semantic_conventions::resource::SERVICE_NAME,
        env!("CARGO_CRATE_NAME"),
    )]);

    let span_exporter = SpanExporter::builder().with_tonic().build()?;

    let tracer_provider = sdk_trace::TracerProvider::builder()
        .with_resource(resource.clone())
        // .with_simple_exporter(span_exporter)
        .with_batch_exporter(span_exporter, runtime::Tokio)
        .build();

    global::set_tracer_provider(tracer_provider.clone());

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
        .init();

    Ok((tracer_provider, logger_provider))
}

#[derive(Clone, Debug)]
struct ServerImpl {
    pool: sqlx::PgPool,
    assets_path: String,
    ctx: ReqwestClient,
    cfg: Config,
    basic_client: BasicClient,
    store: MemoryStore,
    pub employee_dao: EmployeeDao,
}

impl FromRef<ServerImpl> for MemoryStore {
    fn from_ref(state: &ServerImpl) -> Self {
        state.store.clone()
    }
}

impl ServerImpl {
    fn api_employee(
        e: &Employee,
        some_accounts: &Vec<SomeAccount>,
    ) -> skjera_api::models::Employee {
        skjera_api::models::Employee {
            // id: e.id,
            name: e.name.clone(),
            email: e.email.clone(),
            nick: None,
            some_accounts: some_accounts
                .iter()
                .map(ServerImpl::api_some_account)
                .collect(),
        }
    }

    fn api_some_account(s: &SomeAccount) -> skjera_api::models::SomeAccount {
        skjera_api::models::SomeAccount {
            id: s.id.into(),
            network: s.network.to_string(),
            nick: s.nick.to_string(),
            url: s.url.to_string(),
        }
    }
}

async fn start_server(server_impl: ServerImpl, addr: &str) {
    let ap = &server_impl.assets_path.clone();
    let assets_path = Path::new(ap);

    let assets = Router::new().nest_service("/assets", ServeDir::new(assets_path));

    let app = Router::new()
        .route("/", get(html::hello_world))
        .route("/me", get(html::get_me))
        .route("/me", post(html::post_me))
        .route(
            "/me/some_account/:some_account_id/delete",
            post(html::delete_some_account),
        )
        .route("/employee/:employee_id", get(html::employee))
        .route("/oauth/google", get(oauth_google))
        .fallback_service(assets)
        .with_state(server_impl);

    let app = app.layer(TraceLayer::new_for_http());

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
}

impl Config {
    fn new() -> Result<Self, String> {
        let client_id =
            std::env::var("OAUTH_CLIENT_ID").map_err(|_| "OAUTH_CLIENT_ID not set".to_string())?;

        let client_secret = std::env::var("OAUTH_CLIENT_SECRET")
            .map_err(|_| "OAUTH_CLIENT_SECRET not set".to_string())?;

        let redirect_url = std::env::var("OAUTH_REDIRECT_URL")
            .map_err(|_| "OAUTH_REDIRECT_URL not set".to_string())?;

        Ok(Config {
            client_id,
            client_secret,
            redirect_url,
        })
    }
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct AuthRequest {
    code: String,
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

async fn oauth_google(
    Query(query): Query<AuthRequest>,
    State(app): State<ServerImpl>,
) -> Result<impl IntoResponse, AppError> {
    let _method = span!(Level::INFO, "oauth_google");

    let code = query.code;
    debug!("code: {}", code);

    let token = {
        let _span = span!(Level::DEBUG, "exchange_code");
        app.basic_client
            .exchange_code(AuthorizationCode::new(code))
            .request_async(async_http_client)
            .await?
    };

    debug!("token: {:?}", token.scopes());

    let profile = {
        let _span = span!(Level::DEBUG, "get_token");
        app.ctx
            .get("https://openidconnect.googleapis.com/v1/userinfo")
            .bearer_auth(token.access_token().secret().to_owned())
            .send()
            .await?
    };

    // let profile_response = profile.text().await.unwrap();
    // println!("UserProfile: {:?}", profile_response);
    // let user_profile = serde_json::from_str::<UserProfile>(&profile_response).unwrap();

    let user_profile = profile.json::<GoogleUserProfile>().await?;
    info!("UserProfile: {:?}", user_profile);

    let employee = load_or_create_employee(&app, &user_profile).await?;

    let session_user = SessionUser {
        employee: employee.id,
        email: user_profile.email,
        name: user_profile.name,
    };

    // Create a new session filled with user data
    let mut session = Session::new();
    session
        .insert("user", &session_user)
        .context("failed in inserting serialized value into session")?;

    // Store session and get corresponding cookie
    let cookie = app
        .store
        .store_session(session)
        .await
        .context("failed to store session")?
        .context("unexpected error retrieving cookie value")?;

    // Build the cookie
    let cookie = format!("{COOKIE_NAME}={cookie}; SameSite=Lax; HttpOnly; Secure; Path=/");

    // Set cookie
    let mut headers = HeaderMap::new();
    headers.insert(
        SET_COOKIE,
        cookie.parse().context("failed to parse cookie")?,
    );

    Ok((headers, Redirect::to("/")))
}

async fn load_or_create_employee(
    app: &ServerImpl,
    user_profile: &GoogleUserProfile,
) -> Result<Employee, anyhow::Error> {
    let employee = app
        .employee_dao
        .employee_by_email(user_profile.email.clone())
        .await?;

    if let Some(e) = employee {
        info!("Loaded employee user: {:?}", e);
        return Ok(e);
    }

    let employee = app
        .employee_dao
        .insert_employee(user_profile.email.clone(), user_profile.name.clone())
        .await?;

    info!("Created new employee: {:?}", employee);

    Ok(employee)
}

fn build_oauth_client(
    redirect_url: String,
    client_id: String,
    client_secret: String,
) -> BasicClient {
    // If you're not using Google OAuth, you can use whatever the relevant auth/token URL is for your given OAuth service
    let auth_url = AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string())
        .expect("Invalid authorization endpoint URL");
    let token_url = TokenUrl::new("https://www.googleapis.com/oauth2/v3/token".to_string())
        .expect("Invalid token endpoint URL");

    BasicClient::new(
        ClientId::new(client_id),
        Some(ClientSecret::new(client_secret)),
        auth_url,
        Some(token_url),
    )
    .set_redirect_uri(RedirectUrl::new(redirect_url).unwrap())
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GoogleUserProfile {
    sub: String,
    email: String,
    name: String,
    // given_name: Option<String>,
    // family_name: Option<String>,
}

pub struct AuthRedirect;

impl IntoResponse for AuthRedirect {
    fn into_response(self) -> Response {
        Redirect::temporary("/").into_response()
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SessionUser {
    employee: EmployeeId,
    email: String,
    name: String,
}

#[async_trait]
impl<S> FromRequestParts<S> for SessionUser
where
    MemoryStore: FromRef<S>,
    S: Send + Sync,
{
    // If anything goes wrong or no session is found, redirect to the auth page
    type Rejection = AuthRedirect;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let store = MemoryStore::from_ref(state);

        let cookies = parts
            .extract::<TypedHeader<headers::Cookie>>()
            .await
            .map_err(|e| match *e.name() {
                header::COOKIE => match e.reason() {
                    TypedHeaderRejectionReason::Missing => AuthRedirect,
                    _ => panic!("unexpected error getting Cookie header(s): {e}"),
                },
                _ => panic!("unexpected error getting cookies: {e}"),
            })?;
        let session_cookie = cookies.get(COOKIE_NAME).ok_or(AuthRedirect)?;

        let session = store
            .load_session(session_cookie.to_string())
            .await
            .unwrap()
            .ok_or(AuthRedirect)?;

        let user = session.get::<SessionUser>("user").ok_or(AuthRedirect)?;

        Ok(user)
    }
}
