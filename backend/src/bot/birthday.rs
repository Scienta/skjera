use crate::bot::birthday_actor::BirthdayActorMsg;
use crate::bot::birthdays_actor::{BirthdaysActor, BirthdaysActorMsg, CreateBirthdayActor};
use crate::bot::{birthday_actor, SlackHandler, SlackHandlerResponse};
use async_trait::async_trait;
use futures_util::future::RemoteHandle;
use riker::actors::*;
use riker_patterns::ask::ask;
use slack_morphism::prelude::*;
use tracing::info;
use SlackHandlerResponse::*;

#[derive(Clone)]
pub(crate) struct BirthdayHandler {
    system: ActorSystem,
    birthdays_actor: ActorRef<<BirthdaysActor as Actor>::Msg>,
}

impl BirthdayHandler {
    pub(crate) fn new(
        system: ActorSystem,
        birthdays_actor: ActorRef<BirthdaysActorMsg>,
    ) -> BirthdayHandler {
        Self {
            system,
            birthdays_actor,
        }
    }
}

#[async_trait]
impl SlackHandler for BirthdayHandler {
    async fn handle(
        self: &mut Self,
        event: &SlackPushEventCallback,
        body: &SlackMessageEvent,
    ) -> SlackHandlerResponse {
        let (channel, content) = match (
            body.origin.channel.clone(),
            body.content.clone().and_then(|s| s.text),
        ) {
            (Some(channel), Some(content)) => (channel, content),
            _ => return NotHandled,
        };

        let words: Vec<&str> = content.split_whitespace().collect();

        let first = words.get(0);
        let second = words.get(1);

        match (first, second) {
            (Some(&"fake"), Some(&"birthday")) => {
                let (_, content) = words.split_at(2);
                let content = content.join(" ");

                let res: RemoteHandle<ActorRef<BirthdayActorMsg>> = ask(
                    &self.system,
                    &self.birthdays_actor,
                    CreateBirthdayActor {
                        channel: channel.clone(),
                    },
                );

                let addr = res.await;

                info!("new birthday created: {:?}", addr,);

                addr.tell(
                    birthday_actor::Init {
                        content: content.clone(),
                        slack_network_id: event.team_id.clone(),
                    },
                    None,
                );

                Handled
            }
            _ => NotHandled,
        }
    }
}
