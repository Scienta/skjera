use crate::bot::SlackClientSession;
use slack_morphism::prelude::*;
use tracing::{info, warn};

pub(crate) async fn on_hey<'a>(
    session: &SlackClientSession<'a>,
    sender: SlackUserId,
    channel: SlackChannelId,
    content: String,
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
    let message = HelloTemplate { user_id: sender };

    let req = SlackApiChatPostMessageRequest::new(channel, message.render_template());

    match session.chat_post_message(&req).await {
        Ok(_) => (),
        Err(err) => warn!("could not post message: {}", err),
    }
}
