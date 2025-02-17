use anyhow::{anyhow, Error};
use riker::actors::*;
use slack_morphism::events::SlackInteractionBlockActionsEvent;
use slack_morphism::prelude::SlackInteractionActionInfo;
use slack_morphism::SlackActionId;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use tracing::{info, warn};
use uuid::Uuid;

#[actor(AddInteraction, OnInteractionActions)]
pub struct SlackInteractionServer {
    handlers: HashMap<SlackInteractionId, ActorRef<OnInteractionAction>>,
}

impl SlackInteractionServer {
    pub(crate) fn new() -> SlackInteractionServer {
        SlackInteractionServer {
            handlers: HashMap::new(),
        }
    }
}

impl Default for SlackInteractionServer {
    fn default() -> Self {
        Self::new()
    }
}

impl Actor for SlackInteractionServer {
    type Msg = SlackInteractionServerMsg;

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        self.receive(ctx, msg, sender);
    }
}

#[derive(Clone, Debug)]
pub struct AddInteraction {
    pub(crate) recipient: ActorRef<OnInteractionAction>,
}

#[derive(Clone, Debug)]
pub struct AddInteractionResponse {
    pub(crate) interaction_id: SlackInteractionId,
}

impl Receive<AddInteraction> for SlackInteractionServer {
    type Msg = SlackInteractionServerMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: AddInteraction, sender: Sender) {
        let interaction_id = SlackInteractionId::random();

        self.handlers.insert(interaction_id.clone(), msg.recipient);

        let _ = sender
            .unwrap()
            .try_tell(AddInteractionResponse { interaction_id }, ctx.myself.clone());
    }
}

#[derive(Clone, Debug)]
pub struct OnInteractionActions {
    pub(crate) event: SlackInteractionBlockActionsEvent,
}

#[derive(Clone, Debug)]
pub struct OnInteractionAction {
    pub(crate) event: SlackInteractionActionInfo,
}

impl Receive<OnInteractionActions> for SlackInteractionServer {
    type Msg = SlackInteractionServerMsg;

    fn receive(&mut self, _ctx: &Context<Self::Msg>, msg: OnInteractionActions, _sender: Sender) {
        info!("Handling interaction action");

        for action in msg.event.actions.clone().unwrap_or_default().iter() {
            if let Ok(interaction_id) = action.clone().action_id.try_into() {
                match self.handlers.get(&interaction_id) {
                    Some(recipient) => {
                        recipient.do_send(OnInteractionAction {
                            event: action.clone(),
                        });

                        // _ctx.wait(async {
                        //     match recipient
                        //         .send(OnInteractionAction {
                        //             event: action.clone(),
                        //         })
                        //         .await
                        //     {
                        //         Err(err) => warn!("Could not send interaction action: {}", err),
                        //         _ => (),
                        //     };
                        // });
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
