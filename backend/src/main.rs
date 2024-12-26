mod meta;
mod model;
mod skjera;

use sqlx::postgres::PgConnectOptions;
use std::process::exit;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::signal;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=debug", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

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

    let server_impl = ServerImpl { pool };

    start_server(server_impl, "0.0.0.0:8080").await
}

struct ServerImpl {
    pool: sqlx::PgPool,
}

impl ServerImpl {
    fn api_employee(
        e: &model::Employee,
        some_accounts: &Vec<model::SomeAccount>,
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

    fn api_some_account(s: &model::SomeAccount) -> skjera_api::models::SomeAccount {
        skjera_api::models::SomeAccount {
            id: s.id,
            network: s.network.to_string(),
            nick: s.nick.to_string(),
            url: s.url.to_string(),
        }
    }
}

async fn start_server(server_impl: ServerImpl, addr: &str) {
    // Init Axum router
    let app = skjera_api::server::new(Arc::new(server_impl));

    // Add layers to the router
    // let app = app.layer(...);

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
