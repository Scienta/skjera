use crate::model::Employee;
use anyhow::{anyhow, Result};
use async_openai::config::OpenAIConfig;
use async_openai::types::*;
use tracing::{info, instrument};

type Client = async_openai::Client<OpenAIConfig>;

#[derive(Clone)]
pub struct BirthdayBot {
    client: Client,
    assistant_id: String,
}

impl BirthdayBot {
    pub fn new(client: Client, assistant_id: String) -> Self {
        Self {
            client,
            assistant_id,
        }
    }

    #[instrument(skip(self))]
    pub(crate) async fn create_message(self: &Self, e: &Employee) -> Result<String> {
        let input = format!(
            "Lag en morsom \"gratulerer med dagen\"-melding til {} som har bursdag i dag!",
            e.name
        );

        let (run, message) = self.run_message(input).await?;

        info!(run=?run, message=?message);

        Ok(message)
    }

    #[instrument(skip(self))]
    async fn run_message(self: &Self, input: String) -> Result<(RunObject, String)> {
        let thread_request = CreateThreadRequestArgs::default().build()?;
        let thread = self.client.threads().create(thread_request.clone()).await?;

        info!("Created thread {}", thread.id);

        let message = CreateMessageRequestArgs::default()
            .role(MessageRole::User)
            .content(input.clone())
            .build()?;

        let _message_obj = self
            .client
            .threads()
            .messages(&thread.id)
            .create(message)
            .await?;

        info!("Created message {}", _message_obj.id);

        let run_request = CreateRunRequestArgs::default()
            .assistant_id(self.assistant_id.clone())
            .build()?;

        let run = self
            .client
            .threads()
            .runs(&thread.id)
            .create(run_request)
            .await?;

        info!("Created run {}", run.id);

        let query = [("limit", "1")]; //limit the list responses to 1 message

        let mut err = None;
        while err.is_none() {
            //retrieve the run
            let run = self
                .client
                .threads()
                .runs(&thread.id)
                .retrieve(&run.id)
                .await?;
            //check the status of the run
            info!("run status: {:?}", run.status);

            match run.status {
                RunStatus::Completed => {
                    //retrieve the response from the run
                    let response = self
                        .client
                        .threads()
                        .messages(&thread.id)
                        .list(&query)
                        .await?;
                    //get the message id from the response
                    let message_id = response.data.first().unwrap().id.clone();
                    //get the message from the response
                    let message = self
                        .client
                        .threads()
                        .messages(&thread.id)
                        .retrieve(&message_id)
                        .await?;
                    //get the content from the message
                    let content = message.content.first().unwrap();
                    //get the text from the content
                    let text = match content {
                        MessageContent::Text(text) => text.text.value.clone(),
                        MessageContent::ImageFile(_) | MessageContent::ImageUrl(_) => {
                            panic!("imaged are not expected in this example");
                        }
                        MessageContent::Refusal(refusal) => refusal.refusal.clone(),
                    };
                    return Ok((run, text));
                }
                RunStatus::Failed => err = Some(anyhow!("Run railed: {:#?}", run)),
                RunStatus::Queued => {}
                RunStatus::Cancelling => {}
                RunStatus::Cancelled => err = Some(anyhow!("run cancelled")),
                RunStatus::Expired => err = Some(anyhow!("run expired")),
                RunStatus::RequiresAction => err = Some(anyhow!("run expired")),
                RunStatus::InProgress => {}
                RunStatus::Incomplete => err = Some(anyhow!("run incomplete")),
            }
            //wait for 1 second before checking the status again
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }

        // bot.client.threads().delete(&thread.id).await?;

        Err(err.unwrap())
    }
}
