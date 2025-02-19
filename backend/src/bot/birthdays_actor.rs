use crate::birthday_assistant::BirthdayAssistant;
use crate::bot::birthday_actor::{BirthdayActor, BirthdayActorMsg};
use crate::bot::SlackClient;
use crate::model::Dao;
use crate::slack_interaction_server::SlackInteractionServer;
use ractor::{Actor, ActorProcessingErr, ActorRef, RpcReplyPort};
use slack_morphism::SlackChannelId;
use std::sync::Arc;
use tracing::info;
use uuid::Uuid;

pub enum BirthdaysActorMsg {
    CreateBirthdayActor(SlackChannelId, RpcReplyPort<ActorRef<BirthdayActorMsg>>),
}

pub(crate) struct BirthdaysActor {
    dao: Dao,
    birthday_assistant: BirthdayAssistant,
    slack_interaction_actor: ActorRef<<SlackInteractionServer as Actor>::Msg>,
    slack_client: Arc<SlackClient>,
}

impl BirthdaysActor {
    pub fn new(
        dao: Dao,
        birthday_assistant: BirthdayAssistant,
        slack_interaction_actor: ActorRef<<SlackInteractionServer as Actor>::Msg>,
        slack_client: Arc<SlackClient>,
    ) -> Self {
        Self {
            dao,
            birthday_assistant,
            slack_interaction_actor,
            slack_client,
        }
    }
}

pub(crate) struct BirthdaysActorState;

#[ractor::async_trait]
impl Actor for BirthdaysActor {
    type Msg = BirthdaysActorMsg;
    type State = BirthdaysActorState;
    type Arguments = ();

    async fn pre_start(
        &self,
        _: ActorRef<Self::Msg>,
        _: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        Ok(Self::State {})
    }

    async fn handle(
        &self,
        myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        _: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            BirthdaysActorMsg::CreateBirthdayActor(channel, reply) => {
                info!("Creating new BirthdayActor");
                let name = format!("birthday/{}", Uuid::now_v7().to_string());

                let (actor, _join_handle) = myself
                    .spawn_linked(
                        Some(name),
                        BirthdayActor::new(
                            self.dao.clone(),
                            self.birthday_assistant.clone(),
                            self.slack_interaction_actor.clone(),
                            self.slack_client.clone(),
                        ),
                        (channel,),
                    )
                    .await?;

                // TODO: this is there the join handle should be kept or something

                reply.send(actor)?
            }
        }

        Ok(())
    }
}
