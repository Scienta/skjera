use crate::bot::birthday_actor::BirthdayMsg;
use crate::bot::birthday_actors::{BirthdaysActor, CreateBirthdayActor};
use crate::bot::{SlackHandler, SlackHandlerResponse};
use actix::prelude::*;
use async_trait::async_trait;
use slack_morphism::prelude::*;
use SlackHandlerResponse::*;

#[derive(Clone)]
pub(crate) struct BirthdayHandler {
    birthdays_actor: Addr<BirthdaysActor>,
}

impl BirthdayHandler {
    pub(crate) fn new(birthdays_actor: Addr<BirthdaysActor>) -> BirthdayHandler {
        Self { birthdays_actor }
    }
}

#[async_trait]
impl SlackHandler for BirthdayHandler {
    async fn handle(
        self: &mut Self,
        _sender: &SlackUserId,
        channel: &SlackChannelId,
        content: &String,
    ) -> SlackHandlerResponse {
        let words: Vec<&str> = content.split_whitespace().collect();

        let first = words.get(0);
        let second = words.get(1);

        match (first, second) {
            (Some(&"fake"), Some(&"birthday")) => {
                let (_, content) = words.split_at(2);
                let content = content.join(" ");

                let addr = self
                    .birthdays_actor
                    .send(CreateBirthdayActor {
                        channel: channel.clone(),
                    })
                    .await
                    .unwrap();

                let _ = addr.send(BirthdayMsg::Init(content)).await;
                // self.on_msg(session, sender.clone(), channel.clone(), content)
                //     .await
                Handled
            }
            _ => NotHandled,
        }
    }
}
