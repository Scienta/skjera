use crate::oauth::oauth_google;
use crate::{html, slack, ServerImpl};
use axum::routing::{get, post};
use axum::Router;
use std::path::Path;
use tower_http::services::ServeDir;

pub(crate) fn create_router(app: ServerImpl) -> Router {
    let ap = &app.assets_path.clone();
    let assets_path = Path::new(ap);

    let assets = Router::new().nest_service("/assets", ServeDir::new(assets_path));

    Router::new()
        .route("/", get(html::hello_world))
        .route("/me", get(html::get_me))
        .route("/me", post(html::post_me))
        .route("/me/some_account/add", post(html::add_some_account))
        .route(
            "/me/some_account/{some_account_id}/delete",
            post(html::delete_some_account),
        )
        .route("/employee/{employee_id}", get(html::employee))
        .route("/employee/{employee_id}/create-message", get(html::employee_create_message))
        .route("/oauth/google", get(oauth_google))
        .route("/oauth/slack-begin", get(slack::oauth_slack_begin))
        .route("/oauth/slack", get(slack::oauth_slack))
        .fallback_service(assets)
        .with_state(app)
}
