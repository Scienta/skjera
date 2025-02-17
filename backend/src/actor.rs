use actix::dev::{MessageResponse, OneshotSender};
use actix::{Actor, Context, Handler, Message, Recipient};
use anyhow::{anyhow, Error};
use slack_morphism::events::SlackInteractionBlockActionsEvent;
use slack_morphism::SlackActionId;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use tracing::info;
use uuid::Uuid;

pub struct SlackInteractionActor {
    handlers: HashMap<SlackInteractionId, Recipient<OnInteraction>>,
}

impl Actor for SlackInteractionActor {
    type Context = Context<Self>;
}

impl SlackInteractionActor {
    pub(crate) fn new() -> SlackInteractionActor {
        SlackInteractionActor {
            handlers: HashMap::new(),
        }
    }
}

#[derive(Message)]
#[rtype(result = "AddInteractionResponse")]
pub struct AddInteraction {
    pub(crate) recipient: Recipient<OnInteraction>,
}

pub struct AddInteractionResponse {
    pub(crate) interaction_id: SlackInteractionId,
}

impl<A, M> MessageResponse<A, M> for AddInteractionResponse
where
    A: Actor,
    M: Message<Result = AddInteractionResponse>,
{
    fn handle(self, _ctx: &mut A::Context, tx: Option<OneshotSender<M::Result>>) {
        if let Some(tx) = tx {
            let _ = tx.send(self);
        }
    }
}

impl Handler<AddInteraction> for SlackInteractionActor {
    type Result = AddInteractionResponse;

    fn handle(&mut self, msg: AddInteraction, _ctx: &mut Self::Context) -> Self::Result {
        let interaction_id = SlackInteractionId::random();

        self.handlers.insert(interaction_id.clone(), msg.recipient);

        AddInteractionResponse { interaction_id }
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct OnInteraction {
    pub(crate) event: SlackInteractionBlockActionsEvent,
}

impl Handler<OnInteraction> for SlackInteractionActor {
    type Result = ();

    fn handle(&mut self, msg: OnInteraction, _ctx: &mut Self::Context) -> Self::Result {
        info!("Handling interaction event: {:?}", msg.event);
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
    type Error = Error;

    fn try_from(value: SlackActionId) -> anyhow::Result<Self> {
        Uuid::parse_str(&value.0)
            .map(SlackInteractionId)
            .map_err(|e| anyhow!(e))
    }
}
