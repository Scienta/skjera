pub mod birthday;
pub mod birthday_actor;
pub mod birthdays_actor;
pub mod hey;

use crate::slack_interaction_server::{OnInteractionActions, SlackInteractionServer};
use riker::actors::*;
use async_trait::async_trait;
use axum::response::{IntoResponse, Response};
use http::StatusCode;
use slack_morphism::prelude::*;
use sqlx::{Database, Pool};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info, instrument, warn};

// pub(crate) type SlackClientSession<'a> =
//     slack_morphism::SlackClientSession<'a, SlackClientHyperHttpsConnector>;
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
        event: &SlackPushEventCallback,
        body: &SlackMessageEvent,
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
    slack_interaction_actor: ActorRef<<SlackInteractionServer as Actor>::Msg>,
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
            slack_interaction_actor: self.slack_interaction_actor.clone(),
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
        slack_interaction_actor: ActorRef<<SlackInteractionServer as Actor>::Msg>,
    ) -> Self {
        SkjeraBot {
            client,
            pool,
            handlers,
            slack_interaction_actor,
        }
    }

    #[instrument(skip(self, event))]
    pub(crate) async fn on_event<'a>(self: &Self, event: SlackPushEventCallback) -> Response {
        info!("Received slack push event");

        match event.event.clone() {
            SlackEventCallbackBody::Message(body) => self.on_message(event, body).await,
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
        info!("Received slack interaction event");

        self.slack_interaction_actor
            .tell(OnInteractionActions { event }, None);

        (StatusCode::OK, "got it!").into_response()
    }

    async fn on_message<'a>(self: &Self, event: SlackPushEventCallback, body: SlackMessageEvent) {
        match (&body.sender.bot_id, &body.origin.channel_type) {
            (bot_id, Some(SlackChannelType(channel_type))) => {
                if channel_type != "im" {
                    return;
                }

                // This is set if this bot was the sender
                if bot_id.is_some() {
                    debug!("Ignoring own message");
                    return;
                }

                info!("got message: {:?}", body.clone());

                for h in self.handlers.iter() {
                    let r = h.lock().await.handle(&event, &body).await;
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
