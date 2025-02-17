use actix::dev::{MessageResponse, OneshotSender};
use actix::{Actor, Context, Handler, Message, Recipient};
use anyhow::{anyhow, Error};
use slack_morphism::events::SlackInteractionBlockActionsEvent;
use slack_morphism::prelude::SlackInteractionActionInfo;
use slack_morphism::SlackActionId;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use tracing::{info, warn};
use uuid::Uuid;

pub struct SlackInteractionServer {
    handlers: HashMap<SlackInteractionId, Recipient<OnInteractionAction>>,
}

impl Actor for SlackInteractionServer {
    type Context = Context<Self>;
}

impl SlackInteractionServer {
    pub(crate) fn new() -> SlackInteractionServer {
        SlackInteractionServer {
            handlers: HashMap::new(),
        }
    }
}

#[derive(Message)]
#[rtype(result = "AddInteractionResponse")]
pub struct AddInteraction {
    pub(crate) recipient: Recipient<OnInteractionAction>,
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

impl Handler<AddInteraction> for SlackInteractionServer {
    type Result = AddInteractionResponse;

    fn handle(&mut self, msg: AddInteraction, _ctx: &mut Self::Context) -> Self::Result {
        let interaction_id = SlackInteractionId::random();

        self.handlers.insert(interaction_id.clone(), msg.recipient);

        AddInteractionResponse { interaction_id }
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct OnInteractionActions {
    pub(crate) event: SlackInteractionBlockActionsEvent,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct OnInteractionAction {
    pub(crate) event: SlackInteractionActionInfo,
}

impl Handler<OnInteractionActions> for SlackInteractionServer {
    type Result = ();

    fn handle(&mut self, msg: OnInteractionActions, _ctx: &mut Self::Context) -> Self::Result {
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
