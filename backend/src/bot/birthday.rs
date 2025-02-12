use crate::actor::{SkjeraSlackInteractionHandler, SlackInteractionHandlers, SlackInteractionId};
use crate::bot::{SlackClientSession, SlackHandler, SlackHandlerResponse};
use crate::model::employee::EmployeeDao;
use crate::model::{Dao, SLACK};
use async_trait::async_trait;
use slack_morphism::prelude::*;
use sqlx::{Pool, Postgres};
use std::sync::Arc;
use tracing::{info, instrument, warn};
use SlackHandlerResponse::*;

#[derive(Clone)]
pub(crate) struct BirthdayHandler {
    pool: Pool<Postgres>,
    slack_interaction_handlers: SlackInteractionHandlers,
    slack_network_id: String,
}

impl BirthdayHandler {
    pub(crate) fn new(
        pool: Pool<Postgres>,
        slack_interaction_handlers: SlackInteractionHandlers,
        network_id: String,
    ) -> BirthdayHandler {
        Self {
            pool,
            slack_interaction_handlers,
            slack_network_id: network_id,
        }
    }

    #[instrument(skip(self, session, content))]
    async fn on_msg<'a>(
        self: &mut Self,
        session: &SlackClientSession<'a>,
        _sender: SlackUserId,
        channel: SlackChannelId,
        content: String,
    ) -> SlackHandlerResponse {
        info!("got message: {:?}", content);

        let username = content;

        let interaction_id = self
            .slack_interaction_handlers
            .add_handler(Arc::new(BirthdayActor {}))
            .await;

        let dao = Dao::new(self.pool.clone());

        let user_id = match dao.employee_by_name(username.clone()).await {
            Ok(Some(e)) => {
                match dao
                    .some_account_for_network(
                        e.id,
                        SLACK.0.clone(),
                        Some(self.slack_network_id.clone()),
                    )
                    .await
                {
                    Ok(Some(account)) => account.subject.map(|s| Ok(SlackUserId(s))).unwrap(),
                    Ok(None) => Err(username.clone()),
                    Err(e) => {
                        warn!("unable to query: {}", e);
                        return Handled;
                    }
                }
            }
            Ok(None) => Err(username.clone()),
            Err(e) => {
                warn!("unable to query: {}", e);
                return Handled;
            }
        };

        let message = BirthdayMessage {
            username,
            user_id,
            interaction_id,
        };

        let req = SlackApiChatPostMessageRequest::new(channel, message.render_template());

        match session.chat_post_message(&req).await {
            Ok(_) => Handled,
            Err(err) => {
                warn!("could not post message: {}", err);
                NotHandled
            }
        }
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

#[async_trait]
impl SlackHandler for BirthdayHandler {
    async fn handle(
        self: &mut Self,
        session: &SlackClientSession,
        sender: &SlackUserId,
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

                self.on_msg(session, sender.clone(), channel.clone(), content)
                    .await
            }
            _ => NotHandled,
        }
    }
}

struct BirthdayActor {}

#[async_trait]
impl SkjeraSlackInteractionHandler for BirthdayActor {
    async fn on_slack_interaction(self: &Self, event: &SlackInteractionBlockActionsEvent) {
        info!("received slack interaction event: {:?}", event);
    }
}
