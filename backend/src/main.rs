mod employee;
mod html;
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
use reqwest::Client as ReqwestClient;
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgConnectOptions;
use std::path::Path;
use std::process::exit;
use tokio::net::TcpListener;
use tokio::signal;
use tower_http::{services::ServeDir, trace::TraceLayer};
use tracing::{debug, span, Level};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

static COOKIE_NAME: &str = "SESSION";

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=debug", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    dotenv::dotenv().unwrap();

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

    let pool = sqlx::postgres::PgPool::connect_lazy_with(options);
    let assets_path = "backend/assets".to_string();
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

    start_server(server_impl, "0.0.0.0:8080").await
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
            id: s.id,
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
        .route("/employee/:employee_id", get(html::employee))
        .route("/oauth/google", get(oauth_google))
        .fallback_service(assets)
        .with_state(server_impl);

    let app = app.layer(TraceLayer::new_for_http());

    // Run the server with graceful shutdown
    let listener = TcpListener::bind(addr).await.unwrap();
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

    // // let profile_response = profile.text().await.unwrap();
    // // println!("UserProfile: {:?}", profile_response);
    // // let user_profile = serde_json::from_str::<UserProfile>(&profile_response).unwrap();
    //
    let user_profile = profile.json::<GoogleUserProfile>().await?;
    debug!("UserProfile: {:?}", user_profile);

    let session_user = SessionUser {
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
