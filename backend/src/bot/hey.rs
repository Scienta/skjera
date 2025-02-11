use crate::bot::{SlackClientSession, SlackHandler, SlackHandlerResponse};
use async_trait::async_trait;
use slack_morphism::prelude::*;
use tracing::{info, warn};

#[derive(Clone)]
pub(crate) struct HeyHandler {}

impl HeyHandler {
    async fn on_msg<'a>(
        self: &Self,
        session: &SlackClientSession<'a>,
        sender: &SlackUserId,
        channel: &SlackChannelId,
        content: &String,
    ) {
        info!("got message: {:?}", content);

        #[derive(Debug, Clone)]
        pub struct HelloTemplate {
            pub user_id: SlackUserId,
        }

        impl SlackMessageTemplate for HelloTemplate {
            fn render_template(&self) -> SlackMessageContent {
                SlackMessageContent::new()
                    .with_text(format!("Hey {}", self.user_id.to_slack_format()))
                    .with_blocks(slack_blocks![
                        some_into(
                            SlackSectionBlock::new()
                                .with_text(md!("Hey {}", self.user_id.to_slack_format()))
                        ) /*,
                          some_into(SlackDividerBlock::new()),
                          some_into(SlackImageBlock::new(
                              Url::parse("https://www.gstatic.com/webp/gallery3/2_webp_ll.png").unwrap(),
                              "Test Image".into()
                          )),
                          some_into(SlackHeaderBlock::new(pt!("Simple header"))),
                          some_into(SlackActionsBlock::new(slack_blocks![some_into(
                              SlackBlockButtonElement::new(
                                  "simple-message-button".into(),
                                  pt!("Simple button text")
                              )
                          )]))*/
                    ])
            }
        }

        // Use it
        let message = HelloTemplate { user_id: sender.clone() };

        let req = SlackApiChatPostMessageRequest::new(channel.clone(), message.render_template());

        match session.chat_post_message(&req).await {
            Ok(_) => (),
            Err(err) => warn!("could not post message: {}", err),
        }
    }
}

#[async_trait]
impl SlackHandler for HeyHandler {
    async fn handle(
        &mut self,
        session: &SlackClientSession,
        sender: &SlackUserId,
        channel: &SlackChannelId,
        content: &String,
    ) -> SlackHandlerResponse {
        if !content.starts_with("hey") {
            return SlackHandlerResponse::NotHandled;
        }

        self.on_msg(session, sender, channel, content).await;

        SlackHandlerResponse::Handled
    }
}
