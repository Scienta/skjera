use crate::bot::{SlackClientSession, SlackHandler, SlackHandlerResponse};
use crate::model::employee::EmployeeDao;
use crate::model::{Dao, SLACK};
use async_trait::async_trait;
use slack_morphism::prelude::*;
use sqlx::{Pool, Postgres};
use tracing::{info, instrument, warn};
use SlackHandlerResponse::*;

#[derive(Clone)]
pub(crate) struct BirthdayHandler {
    pool: Pool<Postgres>,
    slack_network_id: String,
}

impl BirthdayHandler {
    pub(crate) fn new(pool: Pool<Postgres>, network_id: String) -> BirthdayHandler {
        Self {
            pool,
            slack_network_id: network_id,
        }
    }

    #[instrument(skip(self, session, content))]
    async fn on_msg<'a>(
        self: &Self,
        session: &SlackClientSession<'a>,
        _sender: SlackUserId,
        channel: SlackChannelId,
        content: String,
    ) -> SlackHandlerResponse {
        info!("got message: {:?}", content);

        let username = content;

        let dao = Dao::new(self.pool.clone());

        let user_id = match dao.employee_by_username(username.clone()).await {
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

        let message = BirthdayMessage { username, user_id };

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
}

impl SlackMessageTemplate for BirthdayMessage {
    fn render_template(&self) -> SlackMessageContent {
        SlackMessageContent::new()
            .with_blocks(slack_blocks![
                some_into(SlackHeaderBlock::new(pt!("It's a birthday!! :partying_face: :tada:"))),
                some_into(SlackSectionBlock::new().with_text(md!(
                            "Happy birthday to {} :partying_face: :tada:",
                            self.user_id.clone().map(|u| u.to_slack_format()).unwrap_or_else(|s|s)
                        ))),
                some_into(SlackDividerBlock::new()),
                some_into(SlackActionsBlock::new(slack_blocks![some_into(
                    SlackBlockButtonElement::new(
                        "simple-message-button".into(),
                        pt!("Generate message")
                    )
                )]))
            ])
    }
}

#[async_trait]
impl SlackHandler for BirthdayHandler {
    async fn handle(
        self: &Self,
        session: &SlackClientSession,
        sender: &SlackUserId,
        channel: &SlackChannelId,
        content: &String,
    ) -> SlackHandlerResponse {
        let words: Vec<&str> = content.split_whitespace().collect();

        let first = words.get(0);
        let second = words.get(1);
        let third = words.get(2);

        match (first, second, third) {
            (Some(&"fake"), Some(&"birthday"), Some(username)) => {
                self.on_msg(
                    session,
                    sender.clone(),
                    channel.clone(),
                    username.to_string(),
                )
                .await
            }
            _ => NotHandled,
        }
    }
}
