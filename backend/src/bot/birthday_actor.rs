use crate::birthday_assistant::BirthdayAssistant;
use crate::model::{Dao, Employee, EmployeeDao, SomeAccount, SLACK};
use crate::slack_interaction_server::SlackInteractionServerMsg::AddInteraction;
use crate::slack_interaction_server::{
    InteractionSubscriber, SlackInteractionId, SlackInteractionServer,
};
use anyhow::anyhow;
use ractor::{call, Actor, ActorProcessingErr, ActorRef};
use slack_morphism::prelude::*;
use std::sync::Arc;
use tracing::{info, warn};
use BirthdayActorMsg::Init;

pub(crate) struct BirthdayActor;

pub(crate) struct BirthdayActorState {
    dao: Dao,
    birthday_assistant: BirthdayAssistant,
    slack_interaction_actor: ActorRef<<SlackInteractionServer as Actor>::Msg>,

    slack_client: Arc<crate::bot::SlackClient>,
    channel: SlackChannelId,
    employee: Option<Employee>,
    some_account: Option<SomeAccount>,
}

pub enum BirthdayActorMsg {
    Init(/* content */ String, SlackTeamId),
    OnInteraction(SlackInteractionActionInfo),
}

#[ractor::async_trait]
impl Actor for BirthdayActor {
    type Msg = BirthdayActorMsg;
    type State = BirthdayActorState;
    type Arguments = (
        Dao,
        BirthdayAssistant,
        ActorRef<<SlackInteractionServer as Actor>::Msg>,
        Arc<crate::bot::SlackClient>,
        SlackChannelId,
    );

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        let state = match args {
            (dao, birthday_assistant, slack_interaction_actor, slack_client, channel) => {
                Self::State {
                    dao,
                    birthday_assistant,
                    slack_interaction_actor,
                    slack_client,
                    channel,

                    employee: None,
                    some_account: None,
                }
            }
        };
        Ok(state)
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            Init(content, team) => {
                let interaction_id = call!(
                    state.slack_interaction_actor.clone(),
                    AddInteraction,
                    Box::new(BirthdayActorInteractionSubscriber { actor: _myself })
                )?;

                info!("interaction_id: {:?}", interaction_id);

                info!("got message: {:?}", content);

                let username = content;

                let employee = state
                    .dao
                    .employee_by_name(username.clone())
                    .await
                    .ok()
                    .flatten();

                let some_account = match employee.clone() {
                    Some(e) => {
                        state
                            .dao
                            .some_account_for_network(e.id, SLACK.0.clone(), Some(team.0))
                            .await?
                    }
                    _ => None,
                };

                let message = BirthdayMessage {
                    username,
                    user_id: Err("not found".to_owned()),
                    interaction_id,
                };

                let req = SlackApiChatPostMessageRequest::new(
                    state.channel.clone(),
                    message.render_template(),
                );

                let session = state
                    .slack_client
                    .client
                    .open_session(&state.slack_client.token);

                let _res = session.chat_post_message(&req).await;

                state.employee = employee;
                state.some_account = some_account;

                Ok(())
            }
            BirthdayActorMsg::OnInteraction(event) => {
                info!("got interaction block action: {:?}", event.clone());

                let employee = state.employee.clone();
                let value = event.value.clone();
                let birthday_assistant = state.birthday_assistant.clone();

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

                Ok(())
            }
        }
    }
}

struct BirthdayActorInteractionSubscriber {
    actor: ActorRef<BirthdayActorMsg>,
}

impl InteractionSubscriber for BirthdayActorInteractionSubscriber {
    fn on_interaction(&self, event: SlackInteractionActionInfo) -> anyhow::Result<()> {
        self.actor
            .send_message(BirthdayActorMsg::OnInteraction(event))
            .map_err(|err| anyhow!("{:?}", err))
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
