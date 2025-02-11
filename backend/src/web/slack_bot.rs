use crate::ServerImpl;
use axum::body::Body;
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::Extension;
use http::StatusCode;
use slack_morphism::prelude::*;
use std::sync::Arc;
use tracing::{info, instrument, warn};

#[axum::debug_handler]
pub(super) async fn slack_push_event(
    State(app): State<ServerImpl>,
    Extension(_environment): Extension<Arc<SlackHyperListenerEnvironment>>,
    Extension(event): Extension<SlackPushEvent>,
) -> Response<Body> {
    fn unhandled_event(event: SlackPushEvent) -> Response {
        warn!("unhandled slack push event: {:?}", event);

        StatusCode::UNPROCESSABLE_ENTITY.into_response()
    }

    let bot = match app.bot {
        Some(x) => x,
        _ => return unhandled_event(event),
    };

    match event {
        SlackPushEvent::UrlVerification(event) => on_url_verification(event).await,
        SlackPushEvent::EventCallback(event) => bot.on_event(event).await,
        _ => unhandled_event(event),
    }
}

#[instrument]
async fn on_url_verification(event: SlackUrlVerificationEvent) -> Response {
    info!("on_url_verification event: {:?}", event);
    Response::new(Body::from(event.challenge)).into_response()
}

pub(super) async fn slack_interaction_event(
    State(app): State<ServerImpl>,
    Extension(_environment): Extension<Arc<SlackHyperListenerEnvironment>>,
    Extension(event): Extension<SlackInteractionEvent>,
) -> Response<Body> {
    fn unhandled_event(event: SlackInteractionEvent) -> Response {
        warn!("unhandled slack interaction event: {:?}", event);

        StatusCode::UNPROCESSABLE_ENTITY.into_response()
    }

    let bot = match app.bot {
        Some(x) => x,
        _ => return unhandled_event(event),
    };

    match event {
        SlackInteractionEvent::BlockActions(event) => bot.on_block_action(event).await,
        _ => unhandled_event(event),
    }
}

pub(super) fn slack_error_handler(
    err: Box<dyn std::error::Error + Send + Sync>,
    _client: Arc<SlackHyperClient>,
    _states: SlackClientEventsUserState,
) -> StatusCode {
    info!("Slack error: {:#?}", err);

    StatusCode::BAD_REQUEST
}
