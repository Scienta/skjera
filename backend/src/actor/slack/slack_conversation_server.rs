use ractor::{Actor, ActorProcessingErr, ActorRef, RpcReplyPort};
use slack_morphism::prelude::*;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::marker::PhantomData;

#[async_trait::async_trait]
pub trait Spawn<Msg>: Sized + Sync + Send + 'static
where
    Msg: ractor::Message,
{
    async fn spawn(&self) -> Result<ActorRef<Msg>, ActorProcessingErr>;
}

#[async_trait::async_trait]
pub trait OnPush<Msg>: Sized + Sync + Send + 'static
where
    Msg: ractor::Message,
{
    async fn on_push(
        &self,
        actor: ActorRef<Msg>,
        event: SlackPushEventCallback,
    ) -> Result<(), ActorProcessingErr>;
}

pub struct SlackConversationServer<Msg, Factory>
where
    Msg: ractor::Message + Send + 'static,
    Factory: Spawn<Msg> + OnPush<Msg>,
{
    factory: Factory,
    _phantom_data: PhantomData<Msg>,
}

impl<Msg, Factory> SlackConversationServer<Msg, Factory>
where
    Msg: ractor::Message + Send,
    Factory: Spawn<Msg> + OnPush<Msg>,
{
    pub fn new(factory: Factory) -> SlackConversationServer<Msg, Factory> {
        SlackConversationServer::<Msg, Factory> {
            factory,
            _phantom_data: PhantomData::default(),
        }
    }
}

pub enum SlackConversationServerMsg<Msg>
where
    Msg: ractor::Message,
{
    OnPushEvent {
        team: SlackTeamId,
        channel: SlackChannelId,
        event: SlackPushEventCallback,
    },
    Get {
        team: SlackTeamId,
        channel: SlackChannelId,
        reply: RpcReplyPort<ActorRef<Msg>>,
    },
    Stop {
        team: SlackTeamId,
        channel: SlackChannelId,
        reason: Option<String>,
    },
}

type Key = (SlackTeamId, SlackChannelId);

pub struct SlackConversationServerState<Msg>
where
    Msg: ractor::Message,
{
    conversations: HashMap<Key, ActorRef<Msg>>,
}

impl<Msg, Factory> SlackConversationServer<Msg, Factory>
where
    Msg: ractor::Message + Send + Sync + 'static,
    Factory: Spawn<Msg> + OnPush<Msg>,
{
    async fn get(
        &self,
        conversations: &mut HashMap<Key, ActorRef<Msg>>,
        team: SlackTeamId,
        channel: SlackChannelId,
    ) -> Result<ActorRef<Msg>, ActorProcessingErr> {
        let key: Key = (team, channel);
        let option = conversations.entry(key);

        match option {
            Entry::Occupied(e) => Ok(e.get().clone()),
            Entry::Vacant(_) => {
                let conversation = self.factory.spawn().await?;

                Ok(option.insert_entry(conversation).get().clone())
            }
        }
    }
}

pub struct SlackConversationServerArguments {}

#[ractor::async_trait]
impl<Msg, Factory> Actor for SlackConversationServer<Msg, Factory>
where
    Msg: ractor::Message + Send + Sync + 'static,
    Factory: Spawn<Msg> + OnPush<Msg>,
{
    type Msg = SlackConversationServerMsg<Msg>;
    type State = SlackConversationServerState<Msg>;
    type Arguments = SlackConversationServerArguments;

    async fn pre_start(
        &self,
        _: ActorRef<Self::Msg>,
        _: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        Ok(SlackConversationServerState {
            conversations: HashMap::new(),
        })
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            SlackConversationServerMsg::Get {
                team,
                channel,
                reply,
            } => {
                let conversation = self.get(&mut state.conversations, team, channel).await?;
                reply.send(conversation)?;
                Ok(())
            }
            SlackConversationServerMsg::Stop {
                team,
                channel,
                reason,
            } => match state.conversations.remove(&(team, channel)) {
                Some(e) => Ok(e.stop(reason)),
                None => Ok(()),
            },
            SlackConversationServerMsg::OnPushEvent {
                team,
                channel,
                event: push,
            } => {
                let a = self.get(&mut state.conversations, team, channel).await?;
                self.factory.on_push(a, push).await
            }
        }
    }
}
