use crate::bot::birthday_actor::BirthdayActorMsg::*;
use crate::bot::birthdays_actor::BirthdaysActorMsg;
use crate::bot::birthdays_actor::BirthdaysActorMsg::*;
use crate::bot::{SlackHandler, SlackHandlerResponse};
use async_trait::async_trait;
use ractor::{call_t, cast, ActorRef};
use slack_morphism::prelude::*;
use tracing::{info, warn};
use SlackHandlerResponse::*;

#[derive(Clone)]
pub(crate) struct BirthdayHandler {
    birthdays_actor: ActorRef<BirthdaysActorMsg>,
}

impl BirthdayHandler {
    pub(crate) fn new(birthdays_actor: ActorRef<BirthdaysActorMsg>) -> BirthdayHandler {
        Self { birthdays_actor }
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

                let addr = call_t!(
                    self.birthdays_actor,
                    CreateBirthdayActor,
                    1000,
                    channel.clone()
                )
                .expect("could not start birthday actor");

                info!("new birthday created: {:?}", addr);

                if let Err(e) = cast!(addr, Init(content.clone(), event.team_id.clone())) {
                    warn!("could not initialize birthday actor: {:?}", e);
                }

                Handled
            }
            _ => NotHandled,
        }
    }
}
