use crate::actor::SlackInteractionHandlers;
use crate::birthday_assistant::BirthdayAssistant;
use crate::bot::birthday_actor::BirthdayActor;
use crate::bot::SlackClient;
use crate::model::Dao;
use actix::prelude::*;
use slack_morphism::SlackChannelId;
use std::sync::Arc;

pub(crate) struct BirthdaysActor {
    dao: Dao,
    birthday_assistant: BirthdayAssistant,
    slack_interaction_handlers: SlackInteractionHandlers,
    slack_client: Arc<SlackClient>,
}

#[derive(Message)]
#[rtype(result = "Addr<BirthdayActor>")]
pub(crate) struct CreateBirthdayActor {
    pub channel: SlackChannelId,
}

impl BirthdaysActor {
    pub fn new(
        dao: Dao,
        birthday_assistant: BirthdayAssistant,
        slack_interaction_handlers: SlackInteractionHandlers,
        slack_client: Arc<SlackClient>,
    ) -> Self {
        Self {
            dao,
            birthday_assistant,
            slack_interaction_handlers,
            slack_client,
        }
    }
}

impl Actor for BirthdaysActor {
    type Context = Context<Self>;
}

impl Handler<CreateBirthdayActor> for BirthdaysActor {
    type Result = Addr<BirthdayActor>;

    fn handle(&mut self, msg: CreateBirthdayActor, _ctx: &mut Self::Context) -> Self::Result {
        BirthdayActor::new(
            self.dao.clone(),
            self.birthday_assistant.clone(),
            self.slack_interaction_handlers.clone(),
            self.slack_client.clone(),
            msg.channel,
        )
        .start()
    }
}
