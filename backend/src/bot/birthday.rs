use crate::bot::birthdays_actor::{BirthdaysActor, CreateBirthdayActor};
use crate::bot::{birthday_actor, SlackHandler, SlackHandlerResponse};
use actix::prelude::*;
use async_trait::async_trait;
use slack_morphism::prelude::*;
use tracing::{info, warn};
use SlackHandlerResponse::*;

#[derive(Clone)]
pub(crate) struct BirthdayHandler {
    birthdays_actor: Addr<BirthdaysActor>,
}

impl BirthdayHandler {
    pub(crate) fn new(birthdays_actor: Addr<BirthdaysActor>) -> BirthdayHandler {
        Self { birthdays_actor }
    }

    async fn fake_birthday(
        &mut self,
        channel: &SlackChannelId,
        content: &String,
    ) -> anyhow::Result<()> {
        let addr = self
            .birthdays_actor
            .send(CreateBirthdayActor {
                channel: channel.clone(),
            })
            .await?;

        info!(
            "new birthday created: {:?}, connected={}",
            addr,
            addr.connected()
        );

        let _x = addr
            .send(birthday_actor::Init {
                content: content.clone(),
            })
            .await?;

        info!("Birthday initialized");

        Ok(())
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

                match self.fake_birthday(channel, &content).await {
                    Ok(_) => (),
                    Err(e) => warn!("fake birthday error: {}", e),
                }

                // self.on_msg(session, sender.clone(), channel.clone(), content)
                //     .await
                Handled
            }
            _ => NotHandled,
        }
    }
}
