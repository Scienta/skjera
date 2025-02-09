pub mod birthday;
pub mod hey;

use async_trait::async_trait;
use axum::response::{IntoResponse, Response};
use http::StatusCode;
use slack_morphism::prelude::*;
use sqlx::{Database, Pool};
use std::sync::Arc;
use tracing::{info, instrument, warn};

pub(crate) type SlackClientSession<'a> =
    slack_morphism::SlackClientSession<'a, SlackClientHyperHttpsConnector>;

pub(crate) enum SlackHandlerResponse {
    Handled,
    NotHandled,
}

#[async_trait]
pub(crate) trait SlackHandler {
    async fn handle(
        &self,
        session: &SlackClientSession,
        sender: &SlackUserId,
        channel: &SlackChannelId,
        content: &String,
    ) -> SlackHandlerResponse;
}

pub(crate) struct SkjeraBot<Db>
where
    Db: Database,
    Pool<Db>: Clone,
{
    client: Arc<SlackClient<SlackClientHyperHttpsConnector>>,
    token: SlackApiToken,
    pool: Pool<Db>,
    handlers: Vec<Arc<dyn SlackHandler + Send + Sync>>,
}

impl<Db: Database + Send + Sync> Clone for SkjeraBot<Db>
where
    Pool<Db>: Clone,
{
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            token: self.token.clone(),
            pool: self.pool.clone(),
            handlers: self.handlers.clone(),
        }
    }
}

impl<Db> SkjeraBot<Db>
where
    Db: Database,
    Pool<Db>: Clone,
{
    pub fn new(
        client: Arc<SlackClient<SlackClientHyperHttpsConnector>>,
        token: SlackApiToken,
        pool: Pool<Db>,
        handlers: Vec<Arc<dyn SlackHandler + Send + Sync>>,
    ) -> Self {
        SkjeraBot {
            client,
            token,
            pool,
            handlers,
        }
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

    #[instrument(skip(self, event))]
    pub(crate) async fn on_block_action<'a>(self: &Self, event: SlackInteractionBlockActionsEvent) -> Response {
        info!("Received slack interaction event: {:?}", event);

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

                let token = self.token.clone();
                let session = self.client.open_session(&token);

                for h in self.handlers.iter() {
                    let r = h.handle(&session, &sender, &channel, &content).await;
                    match r {
                        SlackHandlerResponse::NotHandled => {}
                        SlackHandlerResponse::Handled => return,
                    }
                }
            }
            _ => (),
        };
    }
}
