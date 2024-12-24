use std::sync::Arc;
use async_trait::async_trait;
use axum::extract::Host;
use axum::http::Method;
use axum_extra::extract::CookieJar;
use tokio::net::TcpListener;
use tokio::signal;
use skjera_api::apis::skjera::HelloWorldResponse;
use skjera_api::apis::meta::MetaHealthzResponse;

#[tokio::main]
async fn main() {
    println!("Hello, world!");
    start_server("0.0.0.0:8080").await
}

struct ServerImpl {
    // database: sea_orm::DbConn,
}

#[allow(unused_variables)]
#[async_trait]
impl skjera_api::apis::skjera::Skjera for ServerImpl {
    async fn hello_world(&self, method: Method, host: axum::extract::Host, cookies: axum_extra::extract::cookie::CookieJar) -> Result<HelloWorldResponse, String> {
        todo!()
    }
}

#[allow(unused_variables)]
#[async_trait]
impl skjera_api::apis::meta::Meta for ServerImpl {
    async fn meta_healthz(&self, method: Method, host: Host, cookies: CookieJar) -> Result<MetaHealthzResponse, String> {
        todo!()
    }
}

pub async fn start_server(addr: &str) {
    // initialize tracing
    tracing_subscriber::fmt::init();

    // Init Axum router
    let app = skjera_api::server::new(Arc::new(ServerImpl{}));

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
