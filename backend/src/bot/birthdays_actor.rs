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

pub(crate) struct BirthdaysActor;

pub(crate) struct BirthdaysActorState {
    dao: Dao,
    birthday_assistant: BirthdayAssistant,
    slack_interaction_actor: ActorRef<<SlackInteractionServer as Actor>::Msg>,
    slack_client: Arc<SlackClient>,
}

// impl BirthdaysActor {
//     fn new(
//         dao: Dao,
//         birthday_assistant: BirthdayAssistant,
//         slack_interaction_actor: ActorRef<<SlackInteractionServer as Actor>::Msg>,
//         slack_client: Arc<SlackClient>,
//     ) -> Self {
//         Self {
//             dao,
//             birthday_assistant,
//             slack_interaction_actor,
//             slack_client,
//         }
//     }
// }

#[ractor::async_trait]
impl Actor for BirthdaysActor {
    type Msg = BirthdaysActorMsg;
    type State = BirthdaysActorState;
    type Arguments = (
        Dao,
        BirthdayAssistant,
        ActorRef<<SlackInteractionServer as Actor>::Msg>,
        Arc<SlackClient>,
    );

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        let state = match args {
            (dao, birthday_assistant, slack_interaction_actor, slack_client) => Self::State {
                dao,
                birthday_assistant,
                slack_interaction_actor,
                slack_client,
            },
        };

        Ok(state)
    }

    async fn handle(
        &self,
        myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            BirthdaysActorMsg::CreateBirthdayActor(channel, reply) => {
                info!("Creating new BirthdayActor");
                let name = format!("birthday/{}", Uuid::now_v7().to_string());

                let (actor, _join_handle) = myself
                    .spawn_linked(
                        Some(name),
                        BirthdayActor,
                        (
                            state.dao.clone(),
                            state.birthday_assistant.clone(),
                            state.slack_interaction_actor.clone(),
                            state.slack_client.clone(),
                            channel,
                        ),
                    )
                    .await?;

                // TODO: this is there the join handle should be kept or something

                reply.send(actor)?
            }
        }

        Ok(())
    }
}
