use libp2p::identity::Keypair;
use libp2p::Multiaddr;
use ractor::Actor;
use ractor::ActorProcessingErr;
use ractor::ActorRef;
use tokio::task::JoinHandle;

use crate::handle::CtrlHandle;
use crate::Config;
use crate::Event;

pub struct Gossip<M> {
    listener: ractor::ActorRef<M>,
}

impl<M> Gossip<M>
where
    M: From<Event> + ractor::Message,
{
    pub async fn spawn(
        keypair: Keypair,
        addr: Multiaddr,
        config: Config,
        listener: ractor::ActorRef<M>,
    ) -> Result<(ActorRef<Msg>, JoinHandle<()>), ractor::SpawnErr> {
        Actor::spawn(
            None,
            Self { listener },
            Args {
                keypair,
                addr,
                config,
            },
        )
        .await
    }
}

pub struct Args {
    pub keypair: Keypair,
    pub addr: Multiaddr,
    pub config: Config,
}

pub enum State {
    Stopped,
    Running {
        ctrl_handle: CtrlHandle,
        recv_task: JoinHandle<()>,
    },
}

pub enum Msg {
    Broadcast(Vec<u8>),

    // Internal message
    #[doc(hidden)]
    NewEvent(Event),
}

#[ractor::async_trait]
impl<M> Actor for Gossip<M>
where
    M: From<Event> + ractor::Message,
{
    type Msg = Msg;
    type State = State;
    type Arguments = Args;

    async fn pre_start(
        &self,
        myself: ActorRef<Msg>,
        args: Args,
    ) -> Result<State, ActorProcessingErr> {
        let handle = crate::spawn(args.keypair, args.addr, args.config).await?;
        let (mut recv_handle, ctrl_handle) = handle.split();

        let recv_task = tokio::spawn({
            async move {
                while let Some(event) = recv_handle.recv().await {
                    myself.cast(Msg::NewEvent(event)).unwrap(); // FIXME
                }
            }
        });

        Ok(State::Running {
            ctrl_handle,
            recv_task,
        })
    }

    async fn post_start(
        &self,
        _myself: ActorRef<Msg>,
        _state: &mut State,
    ) -> Result<(), ActorProcessingErr> {
        Ok(())
    }

    async fn handle(
        &self,
        _myself: ActorRef<Msg>,
        msg: Msg,
        state: &mut State,
    ) -> Result<(), ActorProcessingErr> {
        let State::Running { ctrl_handle, .. } = state else {
            return Ok(());
        };

        match msg {
            Msg::Broadcast(data) => ctrl_handle.broadcast(data).await?,
            Msg::NewEvent(event) => self.listener.cast(event.into())?,
        }

        Ok(())
    }

    async fn post_stop(
        &self,
        _myself: ActorRef<Msg>,
        state: &mut State,
    ) -> Result<(), ActorProcessingErr> {
        let state = std::mem::replace(state, State::Stopped);

        if let State::Running {
            ctrl_handle,
            recv_task,
        } = state
        {
            ctrl_handle.wait_shutdown().await?;
            recv_task.await?;
        }

        Ok(())
    }
}
