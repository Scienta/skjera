use axum::response::{IntoResponse, Response};
use http::StatusCode;
use crate::web::slack::SlackEvent;

pub(super) fn url_verification(slack_event: SlackEvent) -> Response {
    let x = match slack_event.challenge {
        Some(challenge) => (StatusCode::OK, challenge),
        _ => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "bad challenge".to_string(),
        ),
    };

    x.into_response()
}
