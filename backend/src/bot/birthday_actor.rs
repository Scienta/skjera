use crate::birthday_assistant::BirthdayAssistant;
use crate::model::{Dao, Employee, EmployeeDao, SomeAccount, SLACK};
use crate::slack_interaction_server::SlackInteractionServerMsg::AddInteraction;
use crate::slack_interaction_server::{
    map_err, InteractionSubscriber, SlackInteractionId, SlackInteractionServer,
};
use anyhow::anyhow;
use ractor::concurrency::JoinHandle;
use ractor::{call, Actor, ActorProcessingErr, ActorRef, MessagingErr};
use slack_morphism::prelude::*;
use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};
use BirthdayActorMsg::*;
use BirthdayActorState::*;

pub(crate) struct BirthdayActor {
    dao: Dao,
    birthday_assistant: BirthdayAssistant,
    slack_interaction_actor: ActorRef<<SlackInteractionServer as Actor>::Msg>,
    slack_client: Arc<crate::bot::SlackClient>,
    timeout_duration: Duration,
}

type TimerT = JoinHandle<Result<(), MessagingErr<BirthdayActorMsg>>>;

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
            timeout_duration: Duration::from_secs(10),
        }
    }

    pub(crate) async fn on_init(
        &self,
        myself: ActorRef<BirthdayActorMsg>,
        content: String,
        team: SlackTeamId,
        New { channel }: &New,
    ) -> anyhow::Result<BirthdayActorState> {
        let interaction_id = call!(
            self.slack_interaction_actor,
            AddInteraction,
            Box::new(BirthdayActorInteractionSubscriber {
                actor: myself.clone()
            })
        )?;

        info!("interaction_id: {:?}", interaction_id);

        info!("got message: {:?}", content);

        let username = content;

        let employee = self.dao.employee_by_name(username.clone()).await?;

        let some_account = match employee.clone() {
            Some(e) => {
                self.dao
                    .some_account_for_network(e.id, SLACK.0.clone(), Some(team.0))
                    .await?
            }
            _ => None,
        };

        let message = BirthdayMessage::initial(&username, &some_account, interaction_id);

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

        let timer = myself.send_after(self.timeout_duration, || Timeout);

        Ok(AwaitingSuggestion(AwaitingSuggestion {
            timer,
            channel: channel.clone(),
            username,
            employee,
            some_account,
            ts: res.ts,
        }))
    }

    pub(crate) async fn create_suggestion(
        &self,
        myself: ActorRef<BirthdayActorMsg>,
        event: SlackInteractionActionInfo,
        AwaitingSuggestion {
            timer,
            channel,
            username,
            employee,
            some_account,
            ts,
        }: &AwaitingSuggestion,
    ) -> anyhow::Result<BirthdayActorState> {
        info!("got interaction block action: {:?}", event.clone());

        timer.abort();

        let employee = match employee {
            Some(e) => e,
            None => return Err(anyhow!("employee not found")),
        };

        let updated = match event.value {
            Some(s) if s == "generate-message" => {
                info!("generating message");
                match self.birthday_assistant.create_message(&employee).await {
                    Ok(birthday_message) => {
                        info!("New birthday message: {}", birthday_message);

                        let interaction_id = call!(
                            self.slack_interaction_actor.clone(),
                            AddInteraction,
                            Box::new(BirthdayActorInteractionSubscriber {
                                actor: myself.clone()
                            })
                        )?;

                        let message = BirthdayMessage::suggestion(
                            &username,
                            some_account,
                            interaction_id,
                            &birthday_message,
                        );

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

                        HaveSuggestion(HaveSuggestion {
                            timer: myself.send_after(self.timeout_duration, || Timeout),
                            channel: channel.clone(),
                            username: username.clone(),
                            employee: Some(employee.clone()),
                            some_account: some_account.clone(),
                            ts: ts.clone(),
                            birthday_message,
                        })
                    }
                    Err(e) => {
                        warn!("unable to create message: {}", e);
                        BirthdayActorState::Fail(Fail {})
                    }
                }
            }
            _ => BirthdayActorState::Fail(Fail {}),
        };

        Ok(updated)
    }
}

pub(crate) struct Fail;

pub(crate) struct New {
    channel: SlackChannelId,
}

pub(crate) struct AwaitingSuggestion {
    timer: TimerT,
    channel: SlackChannelId,
    username: String,
    employee: Option<Employee>,
    some_account: Option<SomeAccount>,
    ts: SlackTs,
}

pub(crate) struct HaveSuggestion {
    timer: TimerT,
    channel: SlackChannelId,
    username: String,
    employee: Option<Employee>,
    some_account: Option<SomeAccount>,
    ts: SlackTs,
    birthday_message: String,
}

pub(crate) struct Completed;

pub enum BirthdayActorState {
    Fail(Fail),
    New(New),
    AwaitingSuggestion(AwaitingSuggestion),
    HaveSuggestion(HaveSuggestion),
    Completed(Completed),
}

impl BirthdayActorState {
    fn ts(&self) -> Option<(SlackChannelId, SlackTs)> {
        match self {
            BirthdayActorState::Fail(..) => None,
            New(..) => None,
            BirthdayActorState::Completed(..) => None,
            AwaitingSuggestion(AwaitingSuggestion { channel, ts, .. }) => {
                Some((channel.clone(), ts.clone()))
            }
            HaveSuggestion(HaveSuggestion { channel, ts, .. }) => {
                Some((channel.clone(), ts.clone()))
            }
        }
    }

    fn birthday_message(&self) -> Option<String> {
        match self {
            HaveSuggestion(HaveSuggestion {
                birthday_message, ..
            }) => Some(birthday_message.clone()),
            _ => None,
        }
    }
}

pub enum BirthdayActorMsg {
    Init(String, SlackTeamId),
    OnInteraction(SlackInteractionActionInfo),
    Timeout,
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
            (channel,) => New(New { channel }),
        };
        Ok(state)
    }

    async fn post_stop(
        &self,
        _: ActorRef<Self::Msg>,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        if let Some((channel, ts)) = state.ts() {
            info!("Stopping, ts={}", ts);
            let session = self
                .slack_client
                .client
                .open_session(&self.slack_client.token);

            let req = &SlackApiChatUpdateRequest::new(
                channel.clone(),
                BirthdayMessage::deleted("unknown".to_string(), None, state.birthday_message())
                    .render_template(),
                ts,
            );

            match session.chat_update(&req).await {
                Ok(_) => (),
                Err(e) => error!("Error updating slack message: {}", e),
            };
        } else {
            info!("Stopping")
        }

        Ok(())
    }

    async fn handle(
        &self,
        myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        let internal = match (message, state.deref()) {
            (Init(content, team), New(new)) => self.on_init(myself, content, team, new).await,
            (OnInteraction(event), AwaitingSuggestion(s)) => {
                self.create_suggestion(myself, event, s).await
            }
            (Timeout, state) => {
                info!("User interaction timed out");

                myself.stop(None);

                Ok(BirthdayActorState::Completed(Completed {}))
            }
            _ => {
                let e = anyhow!("Unexpected internal message/state");
                warn!("failed: {}", e);
                return Err(ActorProcessingErr::from(e));
            }
        };

        match internal {
            Ok(internal) => *state = internal,
            Err(e) => {
                warn!("Internal error: {}", e);
                *state = BirthdayActorState::Fail(Fail {});
                return Err(ActorProcessingErr::from(e));
            }
        }

        Ok(())
    }
}

struct BirthdayActorInteractionSubscriber {
    actor: ActorRef<BirthdayActorMsg>,
}

impl InteractionSubscriber for BirthdayActorInteractionSubscriber {
    fn on_interaction(&self, event: SlackInteractionActionInfo) -> Result<(), MessagingErr<()>> {
        self.actor
            .send_message(OnInteraction(event))
            .map_err(map_err)
    }
}

#[derive(Debug, Clone)]
pub struct BirthdayMessage {
    #[allow(dead_code)]
    pub username: String,
    pub user_id: Option<SlackUserId>,
    pub generate_message_id: Option<SlackInteractionId>,
    pub send_id: Option<SlackInteractionId>,

    pub birthday_message: Option<String>,
    pub deleted: bool,
}

impl BirthdayMessage {
    fn initial(
        username: &String,
        some_account: &Option<SomeAccount>,
        generate_message_id: SlackInteractionId,
    ) -> BirthdayMessage {
        BirthdayMessage {
            username: username.clone(),
            user_id: some_account
                .clone()
                .map(|sa| sa.subject)
                .flatten()
                .map(SlackUserId),
            generate_message_id: Some(generate_message_id),
            send_id: None,
            birthday_message: None,
            deleted: false,
        }
    }

    fn suggestion(
        username: &String,
        some_account: &Option<SomeAccount>,
        generate_message_id: SlackInteractionId,
        birthday_message: &String,
    ) -> BirthdayMessage {
        BirthdayMessage {
            username: username.clone(),
            user_id: some_account
                .clone()
                .map(|sa| sa.subject)
                .flatten()
                .map(SlackUserId),
            generate_message_id: Some(generate_message_id),
            send_id: None,
            birthday_message: Some(birthday_message.clone()),
            deleted: false,
        }
    }

    fn deleted(
        username: String,
        some_account: Option<SomeAccount>,
        birthday_message: Option<String>,
    ) -> BirthdayMessage {
        BirthdayMessage {
            username: username.clone(),
            user_id: some_account.map(|sa| sa.subject).flatten().map(SlackUserId),
            generate_message_id: None,
            send_id: None,
            birthday_message,
            deleted: true,
        }
    }
}

impl SlackMessageTemplate for BirthdayMessage {
    fn render_template(&self) -> SlackMessageContent {
        SlackMessageContent::new().with_blocks(slack_blocks![
            some_into(SlackHeaderBlock::new(pt!(
                "It's a birthday!! :partying_face: :tada:"
            ))),
            some_into(SlackSectionBlock::new().with_text(md!(
                "Happy birthday to {} :partying_face: :tada:",
                self.user_id.clone().map(|u| u.to_slack_format()).unwrap_or_else(||self.username.clone())
            ))),
            some_into(SlackDividerBlock::new()),
            optionally_into(self.generate_message_id.is_some() => SlackActionsBlock::new(slack_blocks![
                some_into(SlackBlockButtonElement::new(
                    self.generate_message_id.clone().unwrap().into(),
                    pt!("Generate message")).
                    with_value("generate-message".to_string())
                )
            ])),
            optionally_into(self.birthday_message.is_some() => SlackSectionBlock::new().with_text(md!(
                "Happy birthday message:\n> {}",
                self.birthday_message.clone().unwrap()))
            ),
            optionally_into(self.send_id.is_some() => SlackActionsBlock::new(slack_blocks![
                some_into(SlackBlockButtonElement::new(
                    self.send_id.clone().unwrap().into(),
                    pt!("Send")).
                    with_value("send-message".to_string())
                )
            ])),
            optionally_into(self.deleted => SlackSectionBlock::new().with_text(md!(
                "You snooze, you loose! :alarm_clock:"
            )))
        ])
    }
}
