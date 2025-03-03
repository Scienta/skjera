use crate::bot::SlackClient;
use slack_morphism::prelude::*;
use std::sync::Arc;
use tracing::{info, warn};

pub(crate) struct HeyHandler {
    pub(crate) slack_client: Arc<SlackClient>,
}

impl HeyHandler {
    pub(crate) async fn on_message(
        self: &Self,
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
        let message = HelloTemplate {
            user_id: sender.clone(),
        };

        let req = SlackApiChatPostMessageRequest::new(channel.clone(), message.render_template());

        // let res = self
        //     .slack_client
        //     .client
        //     .run_in_session(&self.slack_client.token, |s| s.chat_post_message(&req))
        //     .await;

        let session = self
            .slack_client
            .client
            .open_session(&self.slack_client.token);

        let res = session.chat_post_message(&req).await;

        match res {
            Ok(_) => (),
            Err(err) => warn!("could not post message: {}", err),
        }
    }
}
