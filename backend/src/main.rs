mod birthday_assistant;
mod bot;
mod logging;
mod macros;
#[cfg(any())]
mod meta;
mod model;
mod session;
#[cfg(any())]
mod skjera;
mod slack_client;
mod slack_interaction_server;
mod web;

use crate::birthday_assistant::BirthdayAssistant;
use crate::bot::birthday::BirthdayHandler;
use crate::bot::birthdays_actor::{BirthdaysActor, BirthdaysActorMsg};
use crate::bot::hey::HeyHandler;
use crate::bot::SlackClient;
use crate::model::*;
use crate::session::SkjeraSessionData;
use crate::slack_interaction_server::SlackInteractionServer;
use crate::web::web::create_router;
use anyhow::anyhow;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::Router;
use axum_login::{login_required, AuthManagerLayerBuilder};
use oauth2::basic::BasicClient;
use ractor::{Actor, ActorRef};
use reqwest::Client as ReqwestClient;
use sqlx::postgres::PgConnectOptions;
use sqlx::{Pool, Postgres};
use std::env;
use std::path::Path;
use std::process::exit;
use std::string::ToString;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::signal;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tower_sessions::cookie::SameSite::Lax;
use tower_sessions::{MemoryStore, SessionManagerLayer, SessionStore};
use tracing::{debug, info, warn};
use web::oauth;
use web::slack::SlackConnect;

const VERSION_INFO: &str = env!("VERSION_INFO");

pub(crate) type AuthSession = axum_login::AuthSession<ServerImpl>;
const LOGIN_PATH: &'static str = "/login";

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

    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    // match run_migrations(options.clone()).await {
    //     Ok(_) => info!("migrations applies"),
    //     Err(err) => warn!("could not apply migrations: {}", err),
    // }

    let pool = sqlx::postgres::PgPool::connect_lazy_with(options);
    let dao = Dao::new(pool.clone());

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

    let (slack_interaction_server, slack_interaction_server_actor) =
        Actor::spawn(None, SlackInteractionServer, ())
            .await
            .expect("Actor failed to start");

    // TODO: Rename to BIRTHDAY_ASSISTANT
    let birthday_bot = env::var("BIRTHDAY_BOT")
        .ok()
        .map(|assistant_id| BirthdayAssistant::new(async_openai::Client::new(), assistant_id));

    let (slack_client, bot, birthdays) = match configure_slack(
        pool.clone(),
        dao.clone(),
        birthday_bot.clone(),
        slack_interaction_server.clone(),
        &cfg.slack_config,
    )
    .await
    {
        Ok(x) => x,
        Err(e) => return println!("could not configure slack: {}", e),
    };

    let server_impl = ServerImpl {
        pool: pool.clone(),
        assets_path,
        ctx,
        cfg,
        basic_client,
        bot,
        slack_client,
        employee_dao: dao,
        slack_connect,
        birthday_bot,
        // slack_interaction_actor,
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

    if let Some((birthdays, birthdays_actor)) = birthdays {
        birthdays.stop(None);
        birthdays_actor.await.unwrap();
    }

    slack_interaction_server.stop(None);
    slack_interaction_server_actor.await.unwrap();

    logging_subsystem.shutdown().await;

    if let Err(e) = r {
        println!("error: {}", e);
    } else {
        println!("Normal exit");
    }
}

async fn configure_slack(
    pool: Pool<Postgres>,
    dao: Dao,
    birthday_assistant: Option<BirthdayAssistant>,
    slack_interaction_actor: ActorRef<<SlackInteractionServer as Actor>::Msg>,
    slack_config: &Option<SlackConfig>,
) -> anyhow::Result<(
    Option<Arc<SlackClient>>,
    Option<bot::SkjeraBot<Postgres>>,
    Option<(ActorRef<BirthdaysActorMsg>, JoinHandle<()>)>,
)> {
    if let (Some(slack_config), Some(birthday_assistant)) = (slack_config, birthday_assistant) {
        let slack_client = slack_morphism::prelude::SlackClient::new(
            slack_morphism::prelude::SlackClientHyperConnector::new()?,
        );

        let slack_client = crate::bot::SlackClient {
            client: slack_client,
            token: slack_config.bot_token.clone(),
        };

        let slack_client = Arc::new(slack_client);

        let mut handlers: Vec<Arc<Mutex<dyn bot::SlackHandler + Send + Sync>>> = Vec::new();

        let (birthdays, birthdays_actor) = Actor::spawn(
            None,
            BirthdaysActor {},
            (
                dao,
                birthday_assistant,
                slack_interaction_actor.clone(),
                slack_client.clone(),
            ),
        )
        .await
        .expect("Actor failed to start");

        let birthday_handler = BirthdayHandler::new(birthdays.clone());
        handlers.push(Arc::new(Mutex::new(birthday_handler)));

        handlers.push(Arc::new(Mutex::new(HeyHandler {
            slack_client: slack_client.clone(),
        })));

        let bot = bot::SkjeraBot::new(
            slack_client.clone(),
            pool,
            handlers,
            slack_interaction_actor,
        );

        Ok((
            Some(slack_client),
            Some(bot),
            Some((birthdays, birthdays_actor)),
        ))
    } else {
        Ok((None, None, None))
    }
}

#[derive(Clone)]
struct ServerImpl {
    /// TODO: Figure out how to best handle the passing of the pool. Right now it is used inside
    /// EmployeeDao, but not anywhere else. I'm not sure if cloning the Pool is ok or not.
    /// Perhaps the EmployeeDao shouldn't use the pool at all and everything should just use this
    /// single reference.
    #[allow(dead_code)]
    pool: Pool<Postgres>,
    assets_path: String,
    ctx: ReqwestClient,
    cfg: Config,
    basic_client: BasicClient,
    bot: Option<bot::SkjeraBot<Postgres>>,
    pub slack_client: Option<Arc<SlackClient>>,
    pub employee_dao: Dao,
    pub slack_connect: Option<SlackConnect>,
    pub birthday_bot: Option<BirthdayAssistant>,
}

impl ServerImpl {
    fn session_data(e: Employee) -> SkjeraSessionData {
        SkjeraSessionData {
            employee: e.id,
            session_hash: Box::new(e.id.0.to_be_bytes()),
            email: e.email,
            name: e.name,
            slack_connect: None,
        }
    }
}

async fn start_server<SS>(
    server_impl: ServerImpl,
    session_layer: SessionManagerLayer<SS>,
    addr: &str,
) -> anyhow::Result<(), AppError>
where
    SS: SessionStore + Clone,
{
    let assets_path = Path::new(&server_impl.assets_path);
    let assets = Router::new().nest_service("/assets", ServeDir::new(assets_path));

    let (public, private) = create_router(&server_impl)?;
    let private = private.route_layer(login_required!(ServerImpl, login_url = LOGIN_PATH));

    let auth_layer = AuthManagerLayerBuilder::new(server_impl.clone(), session_layer).build();

    let app = Router::new()
        .merge(private)
        .merge(public)
        .layer(auth_layer)
        .layer(TraceLayer::new_for_http())
        .fallback_service(assets.clone())
        .with_state(server_impl);

    // Run the server with graceful shutdown
    let listener = TcpListener::bind(addr)
        .await
        .map_err(|e| anyhow!("could not listen on {}: {}", addr, e))
        .map_err(AppError::Anyhow)?;

    info!("skjera is listening on {}", addr);
    // let app = app.into_make_service();
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|e| anyhow!("server error {}", e))
        .map_err(AppError::Anyhow)
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
            env::var("SLACK_SIGNING_SECRET"),
            env::var("SLACK_BOT_TOKEN"),
        ) {
            (
                Ok(client_id),
                Ok(client_secret),
                Ok(redirect_url),
                Ok(signing_secret),
                Ok(bot_token),
            ) => Some(SlackConfig::new(
                client_id,
                client_secret,
                redirect_url,
                signing_secret.into(),
                slack_morphism::prelude::SlackApiToken::new(bot_token.into()),
            )),
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
    signing_secret: slack_morphism::prelude::SlackSigningSecret,
    bot_token: slack_morphism::prelude::SlackApiToken,
}

impl SlackConfig {
    fn new(
        client_id: String,
        client_secret: String,
        redirect_url: String,
        signing_secret: slack_morphism::prelude::SlackSigningSecret,
        bot_token: slack_morphism::prelude::SlackApiToken,
    ) -> Self {
        Self {
            client_id,
            client_secret,
            redirect_url,
            signing_secret,
            bot_token,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),

    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),

    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),

    #[error(transparent)]
    Askama(#[from] askama_axum::Error),

    #[error(transparent)]
    Url(#[from] url::ParseError),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        tracing::error!("Application error: {:#}", self);

        (StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong").into_response()
    }
}

pub struct AuthRedirect;

impl IntoResponse for AuthRedirect {
    fn into_response(self) -> Response {
        Redirect::temporary("/").into_response()
    }
}
