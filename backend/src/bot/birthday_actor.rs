use crate::actor::{SkjeraSlackInteractionHandler, SlackInteractionHandlers, SlackInteractionId};
use crate::birthday_assistant::BirthdayAssistant;
use crate::model::{Dao, EmployeeDao, SLACK};
use actix::prelude::*;
use anyhow::anyhow;
use async_trait::async_trait;
use slack_morphism::prelude::*;
use std::sync::Arc;
use tracing::{info, instrument};

const SCIENTA_SLACK_NETWORK_ID: &str = "T03S4JU33";

pub(crate) struct BirthdayActor {
    dao: Dao,
    birthday_assistant: BirthdayAssistant,
    slack_interaction_handlers: SlackInteractionHandlers,

    slack_client: Arc<crate::bot::SlackClient>,
    channel: SlackChannelId,
}

impl BirthdayActor {
    pub fn new(
        dao: Dao,
        birthday_assistant: BirthdayAssistant,
        slack_interaction_handlers: SlackInteractionHandlers,
        slack_client: Arc<crate::bot::SlackClient>,
        channel: SlackChannelId,
    ) -> Self {
        Self {
            dao,
            birthday_assistant,
            slack_interaction_handlers,
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
}

impl Handler<Init> for BirthdayActor {
    type Result = ResponseActFuture<Self, ()>;

    #[instrument(skip(self))]
    fn handle(&mut self, msg: Init, _: &mut Context<Self>) -> Self::Result {
        let dao = self.dao.clone();
        let channel = self.channel.clone();
        let slack_client = self.slack_client.clone();

        let f = async move {
            info!("got message: {:?}", msg.content);

            let username = msg.content;

            // let interaction_id = self
            //     .slack_interaction_handlers
            //     .add_handler(Arc::new(BirthdayActor { count: 0 }))
            //     .await;

            let interaction_id = SlackInteractionId::random();

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
                        Ok(Some(account)) => {
                            Ok(account.subject.map(|s| Ok(SlackUserId(s))).unwrap())
                        }
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

            let req = SlackApiChatPostMessageRequest::new(
                channel.clone(),
                message.render_template(),
            );

            // let res = self
            //     .slack_client
            //     .run_in_session(|s|async move {
            //         let req = req;
            //         s.chat_post_message(&req) })
            //     .await
            //     .await
            //     .await;

            let session = slack_client
                .client
                .open_session(&slack_client.token);

            let res = session.chat_post_message(&req).await;

            match res {
                Ok(_) => Ok(()),
                Err(err) => Err(anyhow!("could not post message: {}", err)),
            }
        }
        .into_actor(self)
        .map(|_, _, _| ());

        Box::pin(f)

        // Box::pin(async {}.into_actor(self).map(|_, this, _ctx| {
        //     match msg {
        //         BirthdayMsg::Init(content) => {
        //             let x = this.on_init(content);
        //             x.into_actor(this) },
        //     };
        // }))
    }
}

#[async_trait]
impl SkjeraSlackInteractionHandler for BirthdayActor {
    async fn on_slack_interaction(self: &Self, event: &SlackInteractionBlockActionsEvent) {
        info!("received slack interaction event: {:?}", event);
    }
}

#[derive(Debug, Clone)]
pub struct BirthdayMessage {
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
