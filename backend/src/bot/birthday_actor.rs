use crate::birthday_assistant::BirthdayAssistant;
use crate::model::{Dao, Employee, EmployeeDao, SomeAccount, SLACK};
use crate::slack_interaction_server::SlackInteractionServerMsg::AddInteraction;
use crate::slack_interaction_server::{
    InteractionSubscriber, SlackInteractionId, SlackInteractionServer,
};
use anyhow::anyhow;
use ractor::{call, Actor, ActorProcessingErr, ActorRef};
use slack_morphism::prelude::*;
use std::ops::Deref;
use std::sync::Arc;
use tracing::{info, warn};
use BirthdayActorMsg::*;
use BirthdayActorState::*;

pub(crate) struct BirthdayActor {
    dao: Dao,
    birthday_assistant: BirthdayAssistant,
    slack_interaction_actor: ActorRef<<SlackInteractionServer as Actor>::Msg>,
    slack_client: Arc<crate::bot::SlackClient>,
}

impl BirthdayActor {
    pub fn new(
        dao: Dao,
        birthday_assistant: BirthdayAssistant,
        slack_interaction_actor: ActorRef<<SlackInteractionServer as Actor>::Msg>,
        slack_client: Arc<crate::bot::SlackClient>,
    ) -> Self {
        Self {
            dao,
            birthday_assistant,
            slack_interaction_actor,
            slack_client,
        }
    }

    pub(crate) async fn on_init(
        &self,
        myself: ActorRef<BirthdayActorMsg>,
        content: String,
        team: SlackTeamId,
        channel: SlackChannelId,
    ) -> anyhow::Result<BirthdayActorState> {
        let interaction_id = call!(
            self.slack_interaction_actor,
            AddInteraction,
            Box::new(BirthdayActorInteractionSubscriber { actor: myself })
        )?;

        info!("interaction_id: {:?}", interaction_id);

        info!("got message: {:?}", content);

        let username = content;

        let employee = self
            .dao
            .employee_by_name(username.clone())
            .await
            .ok()
            .flatten();

        let some_account = match employee.clone() {
            Some(e) => {
                self.dao
                    .some_account_for_network(e.id, SLACK.0.clone(), Some(team.0))
                    .await?
            }
            _ => None,
        };

        let message = BirthdayMessage {
            username: username.clone(),
            user_id: Err("not found".to_owned()),
            interaction_id,
            birthday_message: None,
        };

        let req = SlackApiChatPostMessageRequest::new(channel.clone(), message.render_template());

        let session = self
            .slack_client
            .client
            .open_session(&self.slack_client.token);

        let res = session.chat_post_message(&req).await?;

        info!(
            "Posted slack message: channel={}, ts={}",
            res.channel, res.ts
        );

        Ok(AwaitingSuggestion(
            channel.clone(),
            username,
            employee,
            some_account,
            res.ts,
        ))
    }

    pub(crate) async fn create_suggestion(
        &self,
        myself: ActorRef<BirthdayActorMsg>,
        event: SlackInteractionActionInfo,
        channel: SlackChannelId,
        username: String,
        employee: Employee,
        some_account: Option<SomeAccount>,
        ts: SlackTs,
    ) -> anyhow::Result<BirthdayActorState> {
        info!("got interaction block action: {:?}", event.clone());

        let updated = match event.value {
            Some(s) if s == "generate-message" => {
                info!("generating message");
                match self.birthday_assistant.create_message(&employee).await {
                    Ok(birthday_message) => {
                        info!("New birthday message: {}", birthday_message);

                        let interaction_id = call!(
                            self.slack_interaction_actor.clone(),
                            AddInteraction,
                            Box::new(BirthdayActorInteractionSubscriber { actor: myself })
                        )?;

                        let message = BirthdayMessage {
                            username: username.clone(),
                            user_id: Err("not found".to_owned()),
                            interaction_id,
                            birthday_message: Some(birthday_message.clone()),
                        };

                        let req = SlackApiChatUpdateRequest::new(
                            channel.clone(),
                            message.render_template(),
                            ts.clone(),
                        );

                        info!("Updating Slack message, channel={}, ts={}", channel, ts);

                        let session = self
                            .slack_client
                            .client
                            .open_session(&self.slack_client.token);

                        let _res = session.chat_update(&req).await;

                        warn!("slack update: {:?}", _res);

                        HaveSuggestion(
                            channel.clone(),
                            username.clone(),
                            Some(employee.clone()),
                            some_account.clone(),
                            ts.clone(),
                            birthday_message,
                        )
                    }
                    Err(e) => {
                        warn!("unable to create message: {}", e);
                        Fail()
                    }
                }
            }
            _ => Fail(),
        };

        Ok(updated)
    }
}

pub(crate) enum BirthdayActorState {
    Fail(),
    New(SlackChannelId),
    AwaitingSuggestion(
        SlackChannelId,
        String,
        Option<Employee>,
        Option<SomeAccount>,
        SlackTs,
    ),
    HaveSuggestion(
        SlackChannelId,
        String,
        Option<Employee>,
        Option<SomeAccount>,
        SlackTs,
        String,
    ),
}

pub enum BirthdayActorMsg {
    Init(String, SlackTeamId),
    OnInteraction(SlackInteractionActionInfo),
}

#[ractor::async_trait]
impl Actor for BirthdayActor {
    type Msg = BirthdayActorMsg;
    type State = BirthdayActorState;
    type Arguments = (SlackChannelId,);

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        let state = match args {
            (channel,) => New(channel),
        };
        Ok(state)
    }

    async fn handle(
        &self,
        myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        let internal = match (message, state.deref().clone()) {
            (Init(content, team), New(channel)) => {
                self.on_init(myself, content, team, channel.clone()).await
            }
            (
                OnInteraction(event),
                AwaitingSuggestion(channel, username, Some(employee), some_account, ts),
            ) => {
                self.create_suggestion(
                    myself,
                    event,
                    channel.clone(),
                    username.clone(),
                    employee.clone(),
                    some_account.clone(),
                    ts.clone(),
                )
                .await
            }
            _ => {
                warn!("Unexpected internal state");
                Ok(Fail())
            }
        };

        *state = internal?;

        Ok(())
    }
}

struct BirthdayActorInteractionSubscriber {
    actor: ActorRef<BirthdayActorMsg>,
}

impl InteractionSubscriber for BirthdayActorInteractionSubscriber {
    fn on_interaction(&self, event: SlackInteractionActionInfo) -> anyhow::Result<()> {
        self.actor
            .send_message(OnInteraction(event))
            .map_err(|err| anyhow!("{:?}", err))
    }
}

#[derive(Debug, Clone)]
pub struct BirthdayMessage {
    #[allow(dead_code)]
    pub username: String,
    pub user_id: Result<SlackUserId, String>,
    pub interaction_id: SlackInteractionId,

    pub birthday_message: Option<String>,
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
            optionally_into(self.birthday_message.is_none() => SlackActionsBlock::new(slack_blocks![
                some_into(SlackBlockButtonElement::new(
                    self.interaction_id.clone().into(),
                    pt!("Generate message")).
                    with_value("generate-message".to_string())
                )
            ])),
            optionally_into(self.birthday_message.is_some() => SlackSectionBlock::new().with_text(md!(
                "Happy birthday message:\n> {}",
                self.birthday_message.clone().unwrap()))
            ),
            optionally_into(self.birthday_message.is_some() => SlackActionsBlock::new(slack_blocks![
                some_into(SlackBlockButtonElement::new(
                    self.interaction_id.clone().into(),
                    pt!("Send")).
                    with_value("send-message".to_string())
                )
            ]))
        ])
    }
}
