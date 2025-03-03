use crate::actor::slack::default_handler::DefaultSlackHandler;
use crate::bot::birthdays_actor::BirthdaysActorMsg;
use crate::bot::birthdays_actor::BirthdaysActorMsg::*;
use crate::bot::hey::HeyHandler;
use ractor::{call, Actor, ActorProcessingErr, ActorRef};
use slack_morphism::prelude::*;
use tracing::*;

pub enum SkjeraConversationMsg {
    SlackPushEventCallback(SlackPushEventCallback),
}

pub struct SkjeraConversation {
    pub(crate) birthdays_actor: ActorRef<BirthdaysActorMsg>,
    pub(crate) hey: HeyHandler,
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
        use SkjeraConversationMsg::*;
        match message {
            SlackPushEventCallback(event) => self.handle_push(event).await,
        }
    }
}

impl DefaultSlackHandler for SkjeraConversation {
    type Msg = SkjeraConversationMsg;
    type State = SkjeraConversationState;

    async fn on_message(
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
