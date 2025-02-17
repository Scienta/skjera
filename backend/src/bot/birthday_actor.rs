use crate::birthday_assistant::BirthdayAssistant;
use crate::bot::birthdays_actor::BirthdaysActor;
use crate::model::{Dao, Employee, EmployeeDao};
use crate::slack_interaction_server::{
    AddInteraction, OnInteractionAction, SlackInteractionId, SlackInteractionServer,
    SlackInteractionServerMsg,
};
use riker::actors::*;
use slack_morphism::prelude::*;
use std::sync::Arc;
use riker_patterns::ask::ask;
use tracing::{info, warn};

#[actor(Init, OnInteractionAction)]
pub(crate) struct BirthdayActor {
    dao: Dao,
    birthday_assistant: BirthdayAssistant,
    slack_interaction_actor: ActorRef<<SlackInteractionServer as Actor>::Msg>,

    slack_client: Arc<crate::bot::SlackClient>,
    channel: SlackChannelId,
    employee: Option<Employee>,
}

impl
    ActorFactoryArgs<(
        Dao,
        BirthdayAssistant,
        ActorRef<<SlackInteractionServer as Actor>::Msg>,
        Arc<crate::bot::SlackClient>,
        SlackChannelId,
    )> for BirthdayActor
{
    fn create_args(
        (dao, birthday_assistant, slack_interaction_actor, slack_client, channel): (
            Dao,
            BirthdayAssistant,
            ActorRef<<SlackInteractionServer as Actor>::Msg>,
            Arc<crate::bot::SlackClient>,
            SlackChannelId,
        ),
    ) -> Self {
        Self {
            dao,
            birthday_assistant,
            slack_interaction_actor,
            slack_client,
            channel,

            employee: None,
        }
    }
}

impl Actor for BirthdayActor {
    type Msg = BirthdayActorMsg;

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        self.receive(ctx, msg, sender);
    }
}

#[derive(Clone, Debug)]
pub struct Init {
    pub(crate) content: String,
    pub(crate) slack_network_id: SlackTeamId,
}

impl Receive<Init> for BirthdayActor {
    type Msg = BirthdayActorMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: Init, sender: Sender) {
        // TODO: there really should be a way of not having to extract all this stuff here
        // Look into ctx.wait()

        let dao = self.dao.clone();
        let channel = self.channel.clone();
        let slack_client = self.slack_client.clone();
        let content = msg.content;

        let add_interaction = AddInteraction {
            recipient: ctx.myself.clone() as ActorRef<OnInteractionAction>,
        };

        ctx.run(async move {
            ask(ctx, &self.slack_interaction_actor, add_interaction).await
        }).map(|interaction_id| {
            let interaction_id = interaction_id.unwrap().interaction_id;

            info!("interaction_id: {:?}", interaction_id);

            async fn y(
                interaction_id: SlackInteractionId,
                content: String,
                dao: Dao,
                msg: Init,
                channel: SlackChannelId,
                slack_client: Arc<crate::bot::SlackClient>,
            ) -> Option<Employee> {
                info!("got message: {:?}", content);

                let username = content;

                // let interaction_id = SlackInteractionId::random();

                let employee = dao.employee_by_name(username.clone()).await.ok().flatten();

                // let user_id = match dao.employee_by_name(username.clone()).await {
                //     Ok(Some(e)) => {
                //         match dao
                //             .some_account_for_network(
                //                 e.id,
                //                 SLACK.0.clone(),
                //                 Some(msg.slack_network_id.to_string()),
                //             )
                //             .await
                //         {
                //             Ok(Some(account)) => Ok(account.subject.map(SlackUserId).unwrap()),
                //             Ok(None) => Err(username.clone()),
                //             Err(e) => return warn!("unable to query: {}", e),
                //         }
                //     }
                //     Ok(None) => Err(username.clone()),
                //     // Err(e) => Err(anyhow!("unable to query: {}", e)),
                //     Err(e) => return warn!("unable to query: {}", e),
                // };

                let message = BirthdayMessage {
                    username,
                    user_id: Err("not found".to_owned()),
                    interaction_id,
                };

                let req = SlackApiChatPostMessageRequest::new(
                    channel.clone(),
                    message.render_template(),
                );

                let session = slack_client.client.open_session(&slack_client.token);

                let _res = session.chat_post_message(&req).await;

                employee
            }
        })
    }
}

impl Receive<OnInteractionAction> for BirthdayActor {
    type Msg = BirthdayActorMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: OnInteractionAction, _sender: Sender) {
        info!("got interaction block action: {:?}", msg.event.clone());

        let employee = self.employee.clone();
        let value = msg.event.value.clone();
        let birthday_assistant = self.birthday_assistant.clone();

        ctx.wait(
            async move {
                match value {
                    Some(s) if s == "generate-message" => {
                        info!("generating message");
                        match employee {
                            None => info!("no employee found"),
                            Some(employee) => {
                                match birthday_assistant.create_message(&employee).await {
                                    Ok(message) => info!("New birthday message: {}", message),
                                    Err(e) => warn!("unable to create message: {}", e),
                                }
                            }
                        }
                    }
                    _ => (),
                };
            },
        )
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
                .with_value("generate-message".to_string())
            )]))
        ])
    }
}
