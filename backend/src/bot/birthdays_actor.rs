use crate::birthday_assistant::BirthdayAssistant;
use crate::bot::birthday_actor::BirthdayActor;
use crate::bot::SlackClient;
use crate::model::Dao;
use crate::slack_interaction_server::SlackInteractionServer;
use riker::actors::*;
use slack_morphism::SlackChannelId;
use std::sync::Arc;
use tracing::info;
use uuid::Uuid;

#[actor(CreateBirthdayActor)]
pub(crate) struct BirthdaysActor {
    dao: Dao,
    birthday_assistant: BirthdayAssistant,
    slack_interaction_actor: ActorRef<<SlackInteractionServer as Actor>::Msg>,
    slack_client: Arc<SlackClient>,
}

#[derive(Clone, Debug)]
pub(crate) struct CreateBirthdayActor {
    pub channel: SlackChannelId,
}

impl
    ActorFactoryArgs<(
        Dao,
        BirthdayAssistant,
        ActorRef<<SlackInteractionServer as Actor>::Msg>,
        Arc<SlackClient>,
    )> for BirthdaysActor
{
    fn create_args(
        (dao, birthday_assistant, slack_interaction_actor, slack_client): (
            Dao,
            BirthdayAssistant,
            ActorRef<<SlackInteractionServer as Actor>::Msg>,
            Arc<SlackClient>,
        ),
    ) -> Self {
        Self {
            dao,
            birthday_assistant,
            slack_interaction_actor,
            slack_client,
        }
    }
}

impl Actor for BirthdaysActor {
    type Msg = BirthdaysActorMsg;

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        self.receive(ctx, msg, sender);
    }
}

impl Receive<CreateBirthdayActor> for BirthdaysActor {
    type Msg = BirthdaysActorMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: CreateBirthdayActor, _sender: Sender) {
        info!("Creating new BirthdayActor");
        let name = format!("birthday/{}", Uuid::now_v7().to_string());
        let _ = ctx
            .system
            .actor_of_args::<BirthdayActor, _>(
                name.as_ref(),
                (
                    self.dao.clone(),
                    self.birthday_assistant.clone(),
                    self.slack_interaction_actor.clone(),
                    self.slack_client.clone(),
                    msg.channel,
                ),
            )
            .unwrap();
    }
}
