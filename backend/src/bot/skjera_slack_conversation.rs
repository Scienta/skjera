use crate::actor::slack_conversation_server::SlackConversationFactory;
use crate::bot::birthdays_actor::BirthdaysActorMsg;
use crate::bot::birthdays_actor::BirthdaysActorMsg::*;
use crate::bot::hey::HeyHandler;
use crate::bot::skjera_slack_conversation::SkjeraConversationMsg::*;
use crate::bot::SlackClient;
use ractor::{call, Actor, ActorProcessingErr, ActorRef};
use slack_morphism::prelude::*;
use std::sync::Arc;
use tracing::*;

pub struct SkjeraSlackConversationFactory {
    birthdays_actor: ActorRef<BirthdaysActorMsg>,
    slack_client: Arc<SlackClient>,
}

impl SkjeraSlackConversationFactory {
    pub fn new(
        birthdays_actor: ActorRef<BirthdaysActorMsg>,
        slack_client: Arc<SlackClient>,
    ) -> Self {
        SkjeraSlackConversationFactory {
            birthdays_actor,
            slack_client,
        }
    }
}

#[async_trait::async_trait]
impl SlackConversationFactory<SkjeraConversationMsg> for SkjeraSlackConversationFactory {
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

    async fn on_push(
        &self,
        actor: ActorRef<SkjeraConversationMsg>,
        event: SlackPushEventCallback,
    ) -> Result<(), ActorProcessingErr> {
        actor.cast(OnPush(event)).map_err(Into::into)
    }
}

pub enum SkjeraConversationMsg {
    OnPush(SlackPushEventCallback),
}

pub struct SkjeraConversation {
    birthdays_actor: ActorRef<BirthdaysActorMsg>,
    hey: HeyHandler,
}

impl SkjeraConversation {
    pub(crate) async fn on_message(
        &self,
        team_id: SlackTeamId,
        event: SlackMessageEvent,
    ) -> Result<(), ActorProcessingErr> {
        let content = event.content.and_then(|s| s.text).unwrap_or("".to_string());

        let words: Vec<&str> = content.trim().split_whitespace().collect();

        let first = words.get(0);
        let second = words.get(1);

        match (first, second, event.sender.user, event.origin.channel) {
            (Some(&"hey"), Some(content), Some(user), Some(channel)) => {
                self.hey
                    .on_message(&user, &channel, &content.to_string())
                    .await;

                Ok(())
            }

            (Some(&"fake"), Some(&"birthday"), Some(_), Some(channel)) => {
                let (_, content) = words.split_at(2);
                let content = content.join(" ");

                let addr = call!(
                    self.birthdays_actor,
                    CreateBirthdayActor,
                    team_id,
                    channel,
                    content.clone()
                )
                .expect("could not start birthday actor");

                info!("new birthday created: {:?}", addr);

                Ok(())
            }
            _ => Ok(()),
        }
    }
}

pub struct SkjeraConversationState {}

#[ractor::async_trait]
impl Actor for SkjeraConversation {
    type Msg = SkjeraConversationMsg;
    type State = SkjeraConversationState;
    type Arguments = ();

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        _: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        Ok(SkjeraConversationState {})
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        _state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            OnPush(SlackPushEventCallback {
                team_id,
                event: SlackEventCallbackBody::Message(event),
                ..
            }) => self.on_message(team_id, event).await,
            _ => Ok(()),
        }
    }
}
