use crate::birthday_assistant::BirthdayAssistant;
use crate::model::{Dao, EmployeeDao, SLACK};
use crate::slack_interaction_server::{
    AddInteraction, OnInteractionAction, SlackInteractionId, SlackInteractionServer,
};
use actix::prelude::*;
use slack_morphism::prelude::*;
use std::sync::Arc;
use tracing::{info, instrument, warn};

pub(crate) struct BirthdayActor {
    dao: Dao,
    birthday_assistant: BirthdayAssistant,
    slack_interaction_actor: Addr<SlackInteractionServer>,

    slack_client: Arc<crate::bot::SlackClient>,
    channel: SlackChannelId,
}

impl BirthdayActor {
    pub fn new(
        dao: Dao,
        birthday_assistant: BirthdayAssistant,
        slack_interaction_actor: Addr<SlackInteractionServer>,
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
}

impl Actor for BirthdayActor {
    type Context = Context<Self>;
}

#[derive(Message, Debug)]
#[rtype(result = "()")]
pub struct Init {
    pub(crate) content: String,
    pub(crate) slack_network_id: SlackTeamId,
}

impl Handler<Init> for BirthdayActor {
    // type Result = ResponseActFuture<Self, ()>;
    // type Result = ResponseFuture<()>;
    type Result = ();

    #[instrument(skip(self))]
    fn handle(&mut self, msg: Init, ctx: &mut Self::Context) -> Self::Result {
        // TODO: there really should be a way of not having to extract all this stuff here
        // Look into ctx.wait()

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
                                    Some(msg.slack_network_id.to_string()),
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

impl Handler<OnInteractionAction> for BirthdayActor {
    type Result = ();

    fn handle(&mut self, msg: OnInteractionAction, _ctx: &mut Self::Context) -> Self::Result {
        info!("got interaction block action: {:?}", msg.event);
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
