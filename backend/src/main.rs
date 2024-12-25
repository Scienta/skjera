mod model;
mod skjera;
mod meta;

use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::signal;

#[tokio::main]
async fn main() {
    println!("Hello, world!");
    start_server("0.0.0.0:8080").await
}

struct ServerImpl {
    employees: Vec<model::Employee>,
}

impl ServerImpl {
    fn api_employee(e: &model::Employee) -> skjera_api::models::Employee {
        skjera_api::models::Employee {
            // id: e.id,
            name: e.name.clone(),
            nick: e.nick.clone(),
            some_accounts: e.some_accounts.iter().map(ServerImpl::api_some_account).collect(),
        }
    }

    fn api_some_account(s: &model::SomeAccount) -> skjera_api::models::SomeAccount {
        skjera_api::models::SomeAccount {
            name: Some("".to_string()),
            nick: Some("".to_string()),
            url: Some("".to_string()),
        }
    }
}

pub async fn start_server(addr: &str) {
    let trygvis: model::Employee = model::Employee::new("Trygve Laugstøl", "trygvis");
    let tobiast: model::Employee = model::Employee::new("Tobias Torrisen", "tobiast");
    let employees: Vec<model::Employee> = vec![trygvis, tobiast];

    // initialize tracing
    tracing_subscriber::fmt::init();

    // Init Axum router
    let app = skjera_api::server::new(Arc::new(ServerImpl {
        employees: employees.clone(),
    }));

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
