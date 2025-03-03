use crate::actor::slack::slack_conversation_server::{OnPush, Spawn};
use crate::bot::birthdays_actor::BirthdaysActorMsg;
use crate::bot::hey::HeyHandler;
use crate::bot::skjera_slack_conversation::SkjeraConversationMsg::*;
use crate::bot::skjera_slack_conversation::{SkjeraConversation, SkjeraConversationMsg};
use crate::bot::SlackClient;
use ractor::{Actor, ActorProcessingErr, ActorRef};
use std::sync::Arc;

pub struct SkjeraConversations {
    birthdays_actor: ActorRef<BirthdaysActorMsg>,
    slack_client: Arc<SlackClient>,
}

impl SkjeraConversations {
    pub fn new(
        birthdays_actor: ActorRef<BirthdaysActorMsg>,
        slack_client: Arc<SlackClient>,
    ) -> Self {
        SkjeraConversations {
            birthdays_actor,
            slack_client,
        }
    }
}

#[async_trait::async_trait]
impl Spawn<SkjeraConversationMsg> for SkjeraConversations {
    async fn spawn(&self) -> Result<ActorRef<SkjeraConversationMsg>, ActorProcessingErr> {
        let (actor, _) = SkjeraConversation::spawn(
            None,
            SkjeraConversation {
                birthdays_actor: self.birthdays_actor.clone(),
                hey: HeyHandler {
                    slack_client: self.slack_client.clone(),
                },
            },
            (),
        )
        .await?;

        Ok(actor)
    }
}

#[async_trait::async_trait]
impl OnPush<SkjeraConversationMsg> for SkjeraConversations {
    async fn on_push(
        &self,
        actor: ActorRef<SkjeraConversationMsg>,
        event: slack_morphism::prelude::SlackPushEventCallback,
    ) -> Result<(), ActorProcessingErr> {
        actor.cast(SlackPushEventCallback(event)).map_err(Into::into)
    }
}
