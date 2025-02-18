use crate::bot::birthday_actor::BirthdayActorMsg;
use ractor::{cast, Actor, ActorProcessingErr, ActorRef, MessagingErr, RpcReplyPort};
use slack_morphism::events::SlackInteractionBlockActionsEvent;
use slack_morphism::prelude::SlackInteractionActionInfo;
use slack_morphism::SlackActionId;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use tracing::{info, warn};
use uuid::Uuid;

pub trait InteractionSubscriber: Send + 'static {
    fn on_interaction(&self, event: SlackInteractionActionInfo) -> anyhow::Result<()>;
}

pub enum SlackInteractionServerMsg {
    AddInteraction(
        Box<dyn InteractionSubscriber>,
        RpcReplyPort<SlackInteractionId>,
    ),
    OnInteractionActions(SlackInteractionBlockActionsEvent),
    // OnInteractionAction(SlackInteractionActionInfo),
}

struct AddInteractionResponse {
    pub interaction_id: SlackInteractionId,
}

pub struct SlackInteractionServer;

pub struct SlackInteractionServerState {
    handlers: HashMap<SlackInteractionId, Box<dyn InteractionSubscriber>>,
}

impl SlackInteractionServerState {
    pub(crate) fn new() -> SlackInteractionServerState {
        SlackInteractionServerState {
            handlers: HashMap::new(),
        }
    }
}

impl Default for SlackInteractionServerState {
    fn default() -> Self {
        Self::new()
    }
}

#[ractor::async_trait]
impl Actor for SlackInteractionServer {
    type Msg = SlackInteractionServerMsg;
    type State = SlackInteractionServerState;
    type Arguments = ();

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        _args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        Ok(SlackInteractionServerState {
            handlers: HashMap::new(),
        })
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            SlackInteractionServerMsg::AddInteraction(subscription, reply) => {
                let interaction_id = SlackInteractionId::random();

                state.handlers.insert(interaction_id.clone(), subscription);

                let _ = reply.send(interaction_id);

                Ok(())
            }
            SlackInteractionServerMsg::OnInteractionActions(event) => {
                info!("Handling interaction action");

                for action in event.actions.clone().unwrap_or_default().iter() {
                    if let Ok(interaction_id) = action.clone().action_id.try_into() {
                        match state.handlers.get(&interaction_id) {
                            Some(recipient) => {
                                recipient.on_interaction(action.clone());

                                // cast!(recipient, OnInteractionAction(action.clone()))?;
                            }
                            None => {
                                warn!(
                                    "No handler registered for interaction action: {:?}",
                                    action.action_id.clone()
                                );
                            }
                        }
                    }
                }

                Ok(())
            }
        }
    }
}

#[derive(Debug, Clone, Eq, Hash, PartialEq)]
pub struct SlackInteractionId(pub Uuid);

impl SlackInteractionId {
    pub(crate) fn random() -> Self {
        SlackInteractionId(Uuid::now_v7())
    }
}

impl From<SlackInteractionId> for SlackActionId {
    fn from(id: SlackInteractionId) -> Self {
        SlackActionId(id.0.to_string()) // Extract the inner UUID and wrap it in SlackActionId
    }
}

impl Display for SlackInteractionId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl TryFrom<SlackActionId> for SlackInteractionId {
    type Error = anyhow::Error;

    fn try_from(value: SlackActionId) -> anyhow::Result<Self> {
        Uuid::parse_str(&value.0)
            .map(SlackInteractionId)
            .map_err(|e| anyhow::anyhow!(e))
    }
}
