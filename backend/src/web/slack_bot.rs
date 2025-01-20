use crate::ServerImpl;
use axum::body::Body;
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::Extension;
use http::StatusCode;
use slack_morphism::prelude::*;
use std::sync::Arc;
use tracing::{info, instrument, warn};

type SlackClientSession<'a> =
    slack_morphism::SlackClientSession<'a, SlackClientHyperHttpsConnector>;

async fn send_hey<'a>(
    session: &SlackClientSession<'a>,
    sender: SlackUserId,
    channel: SlackChannelId,
    content: String,
) {
    info!("got message: {:?}", content);

    #[derive(Debug, Clone /*, Builder*/)]
    pub struct HelloTemplate {
        pub user_id: SlackUserId,
    }

    impl SlackMessageTemplate for HelloTemplate {
        fn render_template(&self) -> SlackMessageContent {
            SlackMessageContent::new()
                .with_text(format!("Hey {}", self.user_id.to_slack_format()))
                .with_blocks(slack_blocks![
                    some_into(
                        SlackSectionBlock::new()
                            .with_text(md!("Hey {}", self.user_id.to_slack_format()))
                    ) /*,
                      some_into(SlackDividerBlock::new()),
                      some_into(SlackImageBlock::new(
                          Url::parse("https://www.gstatic.com/webp/gallery3/2_webp_ll.png").unwrap(),
                          "Test Image".into()
                      )),
                      some_into(SlackHeaderBlock::new(pt!("Simple header"))),
                      some_into(SlackActionsBlock::new(slack_blocks![some_into(
                          SlackBlockButtonElement::new(
                              "simple-message-button".into(),
                              pt!("Simple button text")
                          )
                      )]))*/
                ])
        }
    }

    // Use it
    let message = HelloTemplate { user_id: sender };

    let req = SlackApiChatPostMessageRequest::new(channel, message.render_template());

    match session.chat_post_message(&req).await {
        Ok(_) => (),
        Err(err) => warn!("could not post message: {}", err),
    }
}

async fn on_message<'a>(session: &SlackClientSession<'a>, event: SlackMessageEvent) {
    info!("got message: {:?}", event.clone());

    let content = event.content.and_then(|c| c.text);

    match (
        event.sender.user,
        event.sender.bot_id,
        event.origin.channel,
        event.origin.channel_type,
        content,
    ) {
        (
            Some(sender),
            bot_id,
            Some(channel),
            Some(SlackChannelType(channel_type)),
            Some(content),
        ) => {
            if channel_type != "im" {
                return;
            }

            // This is set if this bot was the sender
            if bot_id.is_some() {
                return;
            }

            send_hey(session, sender, channel, content).await
        }
        _ => (),
    };
}

pub(super) async fn slack_push_event(
    State(app): State<ServerImpl>,
    Extension(_environment): Extension<Arc<SlackHyperListenerEnvironment>>,
    Extension(event): Extension<SlackPushEvent>,
) -> Response<Body> {
    let slack_config = match app.cfg.slack_config {
        Some(x) => x,
        _ => return unhandled_event(event).await,
    };

    let session = _environment.client.open_session(&slack_config.bot_token);

    match event {
        SlackPushEvent::UrlVerification(event) => on_url_verification(event).await,
        SlackPushEvent::EventCallback(event) => on_event(&session, event).await,
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

#[instrument(skip(client, event))]
async fn on_event<'a>(client: &SlackClientSession<'a>, event: SlackPushEventCallback) -> Response {
    info!("Received slack push event");

    match event.event {
        SlackEventCallbackBody::Message(event) => on_message(client, event).await,
        // SlackEventCallbackBody::AppMention(event) => on_app_mention(event),
        _ => {
            warn!("unhandled");
            ()
        }
    };

    (StatusCode::OK, "got it!").into_response()
}

pub(super) fn slack_error_handler(
    err: Box<dyn std::error::Error + Send + Sync>,
    _client: Arc<SlackHyperClient>,
    _states: SlackClientEventsUserState,
) -> StatusCode {
    info!("Slack error: {:#?}", err);

    StatusCode::BAD_REQUEST
}
