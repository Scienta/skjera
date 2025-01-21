use crate::ServerImpl;
use axum::body::Body;
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::Extension;
use http::StatusCode;
use slack_morphism::prelude::*;
use std::sync::Arc;
use tracing::{info, instrument, warn};

pub(super) async fn slack_push_event(
    State(app): State<ServerImpl>,
    Extension(_environment): Extension<Arc<SlackHyperListenerEnvironment>>,
    Extension(event): Extension<SlackPushEvent>,
) -> Response<Body> {
    let bot = match app.bot {
        Some(x) => x,
        _ => return unhandled_event(event).await,
    };

    match event {
        SlackPushEvent::UrlVerification(event) => on_url_verification(event).await,
        SlackPushEvent::EventCallback(event) => bot.on_event(event).await,
        _ => unhandled_event(event).await,
    }
}

async fn unhandled_event(event: SlackPushEvent) -> Response {
    warn!("unhandled slack push event: {:?}", event);

    StatusCode::UNPROCESSABLE_ENTITY.into_response()
}

#[instrument]
async fn on_url_verification(event: SlackUrlVerificationEvent) -> Response {
    info!("on_url_verification event: {:?}", event);
    Response::new(Body::from(event.challenge)).into_response()
}

pub(super) fn slack_error_handler(
    err: Box<dyn std::error::Error + Send + Sync>,
    _client: Arc<SlackHyperClient>,
    _states: SlackClientEventsUserState,
) -> StatusCode {
    info!("Slack error: {:#?}", err);

    StatusCode::BAD_REQUEST
}
