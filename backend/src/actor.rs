use std::fmt::{Display, Formatter};
use anyhow::{anyhow, Error};
use async_trait::async_trait;
use slack_morphism::events::SlackInteractionBlockActionsEvent;
use slack_morphism::SlackActionId;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

#[async_trait]
pub trait SkjeraSlackInteractionHandler {
    async fn on_slack_interaction(self: &Self, event: &SlackInteractionBlockActionsEvent);
}

#[derive(Clone)]
pub struct SlackInteractionHandlers {
    handlers: Arc<Mutex<Vec<SlackInteractionRegistration>>>,
}

impl SlackInteractionHandlers {
    pub(crate) async fn get_handler(
        &self,
        id: SlackInteractionId,
    ) -> Option<Arc<dyn SkjeraSlackInteractionHandler + Send + Sync>> {
        self.handlers
            .lock()
            .await
            .iter()
            .find(|h| h.id.0 == id.0)
            .map(|h| h.handler.clone())
    }
}

impl SlackInteractionHandlers {
    pub(crate) fn new() -> SlackInteractionHandlers {
        SlackInteractionHandlers {
            handlers: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SlackInteractionId(pub uuid::Uuid);

impl From<SlackInteractionId> for SlackActionId {
    fn from(id: SlackInteractionId) -> Self {
        SlackActionId(id.0.to_string()) // Extract the inner UUID and wrap it in SlackActionId
    }
}

impl Display for SlackInteractionId{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl TryFrom<SlackActionId> for SlackInteractionId {
    type Error = Error;

    fn try_from(value: SlackActionId) -> anyhow::Result<Self> {
        Uuid::parse_str(&value.0)
            .map(SlackInteractionId)
            .map_err(|e| anyhow!(e))
    }
}

#[derive(Clone)]
struct SlackInteractionRegistration {
    id: SlackInteractionId,
    handler: Arc<dyn SkjeraSlackInteractionHandler + Send + Sync>,
}

impl SlackInteractionHandlers {
    pub(crate) async fn add_handler(
        &mut self,
        handler: Arc<dyn SkjeraSlackInteractionHandler + Send + Sync>,
    ) -> SlackInteractionId {
        let id = SlackInteractionId(uuid::Uuid::now_v7());

        self.handlers
            .lock()
            .await
            .push(SlackInteractionRegistration {
                id: id.clone(),
                handler,
            });

        id
    }
}
