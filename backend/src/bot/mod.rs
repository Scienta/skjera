mod hey;

use std::sync::Arc;
use axum::response::{IntoResponse, Response};
use http::StatusCode;
use slack_morphism::prelude::*;
use tracing::{info, instrument, warn};

pub(crate) type SlackClientSession<'a> =
    slack_morphism::SlackClientSession<'a, SlackClientHyperHttpsConnector>;

#[derive(Clone)]
pub(crate) struct SkjeraBot {
    client: Arc<SlackClient<SlackClientHyperHttpsConnector>>,
    token: SlackApiToken,
}

impl SkjeraBot {
    pub fn new(client: Arc<SlackClient<SlackClientHyperHttpsConnector>>, token: SlackApiToken) -> Self {
        SkjeraBot { client, token }
    }

    #[instrument(skip(self, event))]
    pub(crate) async fn on_event<'a>(self: &Self, event: SlackPushEventCallback) -> Response {
        info!("Received slack push event");

        match event.event {
            SlackEventCallbackBody::Message(event) => self.on_message(event).await,
            // SlackEventCallbackBody::AppMention(event) => on_app_mention(event),
            _ => {
                warn!("unhandled");
                ()
            }
        };

        (StatusCode::OK, "got it!").into_response()
    }

    async fn on_message<'a>(self: &Self, event: SlackMessageEvent) {
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

                let session = self.client.open_session(&self.token);

                if content == "hey" {
                    hey::on_hey(&session, sender, channel, content).await;
                }
            }
            _ => (),
        };
    }
}
