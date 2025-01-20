use axum::body::Body;
use axum::response::Response;
use axum::Extension;
use http::StatusCode;
use slack_morphism::events::SlackPushEvent;
use slack_morphism::hyper_tokio::{SlackHyperClient, SlackHyperListenerEnvironment};
use slack_morphism::listener::SlackClientEventsUserState;
use std::sync::Arc;
use tracing::info;

pub(super) async fn slack_push_event(
    Extension(_environment): Extension<Arc<SlackHyperListenerEnvironment>>,
    Extension(event): Extension<SlackPushEvent>,
) -> Response<Body> {
    info!("Received push event: {:?}", event);

    match event {
        SlackPushEvent::UrlVerification(url_ver) => Response::new(Body::from(url_ver.challenge)),
        _ => Response::new(Body::empty()),
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
