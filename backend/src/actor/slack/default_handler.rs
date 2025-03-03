use ractor::ActorProcessingErr;
use slack_morphism::prelude::*;

pub trait DefaultSlackHandler {
    type Msg;
    type State;

    async fn on_message(
        &self,
        team_id: SlackTeamId,
        event: SlackMessageEvent,
    ) -> Result<(), ActorProcessingErr>;

    async fn handle_push(&self, message: SlackPushEventCallback) -> Result<(), ActorProcessingErr> {
        match message {
            SlackPushEventCallback {
                team_id,
                event: SlackEventCallbackBody::Message(event),
                ..
            } => self.on_message(team_id, event).await,
            _ => Ok(()),
        }
    }
}
