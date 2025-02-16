pub mod birthday;
pub mod birthday_actor;
pub mod birthday_actors;
pub mod hey;

use crate::actor::{SkjeraSlackInteractionHandler, SlackInteractionHandlers, SlackInteractionId};
use anyhow::anyhow;
use async_trait::async_trait;
use axum::response::{IntoResponse, Response};
use http::StatusCode;
use slack_morphism::prelude::*;
use sqlx::{Database, Pool};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info, instrument, warn};

pub(crate) type SlackClientSession<'a> =
    slack_morphism::SlackClientSession<'a, SlackClientHyperHttpsConnector>;
// pub(crate) type SlackClient =
//     slack_morphism::SlackClient<SlackClientHyperHttpsConnector>;

pub(crate) type SlackClient = SlackClientWrapper;

#[derive(Clone)]
pub(crate) struct SlackClientWrapper {
    pub(crate) client: slack_morphism::SlackClient<SlackClientHyperHttpsConnector>,
    pub(crate) token: SlackApiToken,
}

// impl<'a> SlackClientWrapper {
//     pub(crate) async fn run_in_session<'b, FN, F, T>(&self, f: FN) -> T
//     where
//         FN: Fn(SlackClientSession<'b>) -> F,
//         F: std::future::Future<Output = T>,
//     {
//         // let session = self.client.open_session::<'a>(&self.token);
//         // f(session)
//
//         self.client.run_in_session::<'b>(&self.token.clone(), f).await
//     }
// }

pub(crate) enum SlackHandlerResponse {
    Handled,
    NotHandled,
}

#[async_trait]
pub(crate) trait SlackHandler {
    async fn handle(
        &mut self,
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
    client: Arc<SlackClient>,
    pool: Pool<Db>,
    handlers: Vec<Arc<Mutex<dyn SlackHandler + Send + Sync>>>,
    slack_interaction_handlers: SlackInteractionHandlers,
}

impl<Db: Database + Send + Sync> Clone for SkjeraBot<Db>
where
    Pool<Db>: Clone,
{
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            pool: self.pool.clone(),
            handlers: self.handlers.clone(),
            slack_interaction_handlers: self.slack_interaction_handlers.clone(),
        }
    }
}

impl<Db> SkjeraBot<Db>
where
    Db: Database,
    Pool<Db>: Clone,
{
    pub fn new(
        client: Arc<SlackClient>,
        pool: Pool<Db>,
        handlers: Vec<Arc<Mutex<dyn SlackHandler + Send + Sync>>>,
        slack_interaction_handlers: SlackInteractionHandlers,
    ) -> Self {
        SkjeraBot {
            client,
            pool,
            handlers,
            slack_interaction_handlers,
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
    pub(crate) async fn on_block_action<'a>(
        self: &Self,
        event: SlackInteractionBlockActionsEvent,
    ) -> Response {
        info!("Received slack interaction event: {:?}", event);

        async fn get_handler(
            action: &SlackInteractionActionInfo,
            slack_interaction_handlers: &SlackInteractionHandlers,
        ) -> anyhow::Result<Arc<dyn SkjeraSlackInteractionHandler + Send + Sync>> {
            let interaction_id: SlackInteractionId = action
                .action_id
                .clone()
                .try_into()
                .map_err(|e| anyhow!("invalid interaction id: {}", e))?;

            slack_interaction_handlers
                .get_handler(interaction_id.clone())
                .await
                .ok_or_else(|| {
                    anyhow!(
                        "No interaction handler for interaction_id: {}",
                        interaction_id.clone()
                    )
                })
        }

        for action in event.actions.clone().unwrap_or_default().iter() {
            match get_handler(action, &self.slack_interaction_handlers).await {
                Ok(h) => h.on_slack_interaction(&event).await,
                Err(e) => {
                    return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
                }
            }
        }

        (StatusCode::OK, "got it!").into_response()
    }

    async fn on_message<'a>(self: &Self, event: SlackMessageEvent) {
        let content = event.content.clone().and_then(|c| c.text);

        match (
            &event.sender.user,
            &event.sender.bot_id,
            &event.origin.channel,
            &event.origin.channel_type,
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
                    debug!("Ignoring own message");
                    return;
                }

                info!("got message: {:?}", event.clone());

                for h in self.handlers.iter() {
                    let r = h.lock().await.handle(&sender, &channel, &content).await;
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
