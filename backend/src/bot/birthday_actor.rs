use crate::actor::{AddInteraction, OnInteraction, SlackInteractionActor, SlackInteractionId};
use crate::birthday_assistant::BirthdayAssistant;
use crate::model::{Dao, EmployeeDao, SLACK};
use actix::prelude::*;
use anyhow::anyhow;
use slack_morphism::prelude::*;
use std::sync::Arc;
use tracing::{info, instrument, warn};

const SCIENTA_SLACK_NETWORK_ID: &str = "T03S4JU33";

pub(crate) struct BirthdayActor {
    dao: Dao,
    birthday_assistant: BirthdayAssistant,
    slack_interaction_actor: Addr<SlackInteractionActor>,

    slack_client: Arc<crate::bot::SlackClient>,
    channel: SlackChannelId,
}

impl BirthdayActor {
    pub fn new(
        dao: Dao,
        birthday_assistant: BirthdayAssistant,
        slack_interaction_actor: Addr<SlackInteractionActor>,
        slack_client: Arc<crate::bot::SlackClient>,
        channel: SlackChannelId,
    ) -> Self {
        Self {
            dao,
            birthday_assistant,
            slack_interaction_actor,
            slack_client,
            channel,
        }
    }

    #[allow(dead_code)]
    async fn on_init(&self, content: String) -> anyhow::Result<()> {
        info!("got message: {:?}", content);

        let username = content;

        // let interaction_id = self
        //     .slack_interaction_handlers
        //     .add_handler(Arc::new(BirthdayActor { count: 0 }))
        //     .await;

        let interaction_id = SlackInteractionId::random();

        let user_id = match self.dao.employee_by_name(username.clone()).await {
            Ok(Some(e)) => {
                match self
                    .dao
                    .some_account_for_network(
                        e.id,
                        SLACK.0.clone(),
                        Some(SCIENTA_SLACK_NETWORK_ID.to_string()),
                    )
                    .await
                {
                    Ok(Some(account)) => Ok(account.subject.map(|s| Ok(SlackUserId(s))).unwrap()),
                    Ok(None) => Ok(Err(username.clone())),
                    Err(e) => Err(anyhow!("unable to query: {}", e)),
                }
            }
            Ok(None) => Ok(Err(username.clone())),
            Err(e) => Err(anyhow!("unable to query: {}", e)),
        }?;

        let message = BirthdayMessage {
            username,
            user_id,
            interaction_id,
        };

        let req =
            SlackApiChatPostMessageRequest::new(self.channel.clone(), message.render_template());

        // let res = self
        //     .slack_client
        //     .run_in_session(|s|async move {
        //         let req = req;
        //         s.chat_post_message(&req) })
        //     .await
        //     .await
        //     .await;

        let session = self
            .slack_client
            .client
            .open_session(&self.slack_client.token);

        let res = session.chat_post_message(&req).await;

        match res {
            Ok(_) => Ok(()),
            Err(err) => Err(anyhow!("could not post message: {}", err)),
        }
    }
}

impl Actor for BirthdayActor {
    type Context = Context<Self>;
}

#[derive(Message, Debug)]
#[rtype(result = "()")]
pub struct Init {
    pub(crate) content: String,
}

impl Handler<Init> for BirthdayActor {
    // type Result = ResponseActFuture<Self, ()>;
    // type Result = ResponseFuture<()>;
    type Result = ();

    #[instrument(skip(self))]
    fn handle(&mut self, msg: Init, ctx: &mut Self::Context) -> Self::Result {
        let dao = self.dao.clone();
        let channel = self.channel.clone();
        let slack_client = self.slack_client.clone();
        let content = msg.content;

        let x = self
            .slack_interaction_actor
            .send(AddInteraction {
                recipient: ctx.address().recipient(),
            })
            .into_actor(self)
            .then(|interaction_id, _this, _ctx| {
                let interaction_id = interaction_id.unwrap().interaction_id;

                info!("interaction_id: {:?}", interaction_id);

                let y = async move {
                    info!("got message: {:?}", content);

                    let username = content;

                    // let interaction_id = SlackInteractionId::random();

                    let user_id = match dao.employee_by_name(username.clone()).await {
                        Ok(Some(e)) => {
                            match dao
                                .some_account_for_network(
                                    e.id,
                                    SLACK.0.clone(),
                                    Some(SCIENTA_SLACK_NETWORK_ID.to_string()),
                                )
                                .await
                            {
                                Ok(Some(account)) => Ok(account.subject.map(SlackUserId).unwrap()),
                                Ok(None) => Err(username.clone()),
                                Err(e) => return warn!("unable to query: {}", e),
                            }
                        }
                        Ok(None) => Err(username.clone()),
                        // Err(e) => Err(anyhow!("unable to query: {}", e)),
                        Err(e) => return warn!("unable to query: {}", e),
                    };

                    let message = BirthdayMessage {
                        username,
                        user_id,
                        interaction_id,
                    };

                    let req = SlackApiChatPostMessageRequest::new(
                        channel.clone(),
                        message.render_template(),
                    );

                    let session = slack_client.client.open_session(&slack_client.token);

                    let _res = session.chat_post_message(&req).await;
                };

                Box::pin(y).into_actor(_this)
            });

        // Box::pin(x)
        ctx.wait(x);
    }
}

impl Handler<OnInteraction> for BirthdayActor {
    type Result = ();

    fn handle(&mut self, msg: OnInteraction, _ctx: &mut Self::Context) -> Self::Result {
        info!("got interaction block actions: {:?}", msg.event);
    }
}

#[derive(Debug, Clone)]
pub struct BirthdayMessage {
    #[allow(dead_code)]
    pub username: String,
    pub user_id: Result<SlackUserId, String>,
    pub interaction_id: SlackInteractionId,
}

impl SlackMessageTemplate for BirthdayMessage {
    fn render_template(&self) -> SlackMessageContent {
        SlackMessageContent::new().with_blocks(slack_blocks![
            some_into(SlackHeaderBlock::new(pt!(
                "It's a birthday!! :partying_face: :tada:"
            ))),
            some_into(SlackSectionBlock::new().with_text(md!(
                            "Happy birthday to {} :partying_face: :tada:",
                            self.user_id.clone().map(|u| u.to_slack_format()).unwrap_or_else(|s|s)
                        ))),
            some_into(SlackDividerBlock::new()),
            some_into(SlackActionsBlock::new(slack_blocks![some_into(
                SlackBlockButtonElement::new(
                    self.interaction_id.clone().into(),
                    pt!("Generate message")
                )
            )]))
        ])
    }
}
