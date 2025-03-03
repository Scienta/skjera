pub mod birthday_actor;
pub mod birthdays_actor;
pub mod hey;
pub mod skjera_slack_conversation;
pub mod skjera_slack_conversations;

use crate::actor::slack::slack_conversation_server::SlackConversationServerMsg;
use crate::bot::skjera_slack_conversation::*;
use crate::slack_interaction_server::SlackInteractionServer;
use crate::slack_interaction_server::SlackInteractionServerMsg::OnInteractionActions;
use axum::response::{IntoResponse, Response};
use http::StatusCode;
use ractor::{cast, Actor, ActorRef};
use slack_morphism::prelude::*;
use sqlx::{Database, Pool};
use std::sync::Arc;
use tracing::*;

pub(crate) type SlackClient = SlackClientWrapper;

#[derive(Clone)]
pub(crate) struct SlackClientWrapper {
    pub(crate) client: slack_morphism::SlackClient<SlackClientHyperHttpsConnector>,
    pub(crate) token: SlackApiToken,
}

pub(crate) struct SkjeraBot<Db>
where
    Db: Database,
    Pool<Db>: Clone,
{
    client: Arc<SlackClient>,
    pool: Pool<Db>,
    slack_interaction_actor: ActorRef<<SlackInteractionServer as Actor>::Msg>,
    slack_conversation_server: ActorRef<SlackConversationServerMsg<SkjeraConversationMsg>>,
}

impl<Db: Database + Send + Sync> Clone for SkjeraBot<Db>
where
    Pool<Db>: Clone,
{
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            pool: self.pool.clone(),
            slack_interaction_actor: self.slack_interaction_actor.clone(),
            slack_conversation_server: self.slack_conversation_server.clone(),
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
        slack_interaction_actor: ActorRef<<SlackInteractionServer as Actor>::Msg>,
        slack_conversation_server: ActorRef<SlackConversationServerMsg<SkjeraConversationMsg>>,
    ) -> Self {
        SkjeraBot {
            client,
            pool,
            slack_interaction_actor,
            slack_conversation_server,
        }
    }

    #[instrument(skip(self, event))]
    pub(crate) async fn on_event<'a>(self: &Self, event: SlackPushEventCallback) -> Response {
        trace!("Received slack push event");

        match &event.event {
            SlackEventCallbackBody::Message(body) if body.origin.channel.is_some() => {
                let event = SlackConversationServerMsg::<SkjeraConversationMsg>::OnPushEvent {
                    team: event.team_id.clone(),
                    channel: body.origin.channel.clone().unwrap(),
                    event,
                };

                match self.slack_conversation_server.cast(event) {
                    Ok(_) => (StatusCode::OK, "got it!").into_response(),
                    Err(_) => {
                        // warn!("Slack conversation error: {:?}", e);
                        (StatusCode::INTERNAL_SERVER_ERROR, "boo").into_response()
                    }
                }
            }
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "boo").into_response(),
        }
    }

    #[instrument(skip(self, event))]
    pub(crate) async fn on_block_action<'a>(
        self: &Self,
        event: SlackInteractionBlockActionsEvent,
    ) -> Response {
        info!("Received slack interaction event");

        if let Err(e) = cast!(self.slack_interaction_actor, OnInteractionActions(event)) {
            warn!("Could not forward event: {}", e);
        }

        (StatusCode::OK, "got it!").into_response()
    }
}
