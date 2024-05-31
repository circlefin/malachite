use async_trait::async_trait;
use ractor::{Actor, ActorRef};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use malachite_common::{Context, Round};
use malachite_proto::Protobuf;
use malachite_vote::ThresholdParams;

use crate::consensus::Msg as ConsensusMsg;
use crate::gossip::Msg as GossipMsg;
use crate::gossip_mempool::Msg as GossipMempoolMsg;
use crate::host::Msg as HostMsg;
use crate::mempool::Msg as MempoolMsg;
use crate::timers::Config as TimersConfig;
use crate::util::ValueBuilder;

pub struct Params<Ctx: Context> {
    pub address: Ctx::Address,
    pub initial_validator_set: Ctx::ValidatorSet,
    pub keypair: malachite_gossip::Keypair,
    pub start_height: Ctx::Height,
    pub threshold_params: ThresholdParams,
    pub timers_config: TimersConfig,
    pub value_builder: Box<dyn ValueBuilder<Ctx>>,
    pub gossip_mempool: ActorRef<crate::gossip_mempool::Msg>,
    pub mempool: ActorRef<crate::mempool::Msg>,
    pub tx_decision: mpsc::Sender<(Ctx::Height, Round, Ctx::Value)>,
}

// pub async fn spawn<Ctx>(
//     ctx: Ctx,
//     params: Params<Ctx>,
// ) -> Result<(ActorRef<Msg>, JoinHandle<()>), ractor::ActorProcessingErr>
// where
//     Ctx: Context,
//     Ctx::Vote: Protobuf<Proto = malachite_proto::Vote>,
//     Ctx::Proposal: Protobuf<Proto = malachite_proto::Proposal>,
// {
//     let cal = CAL::spawn(ctx.clone(), params.initial_validator_set.clone()).await?;
//
//     let proposal_builder = ProposalBuilder::spawn(ctx.clone(), params.value_builder).await?;
//
//     let consensus_params = ConsensusParams {
//         start_height: params.start_height,
//         initial_validator_set: params.initial_validator_set.clone(),
//         address: params.address,
//         threshold_params: params.threshold_params,
//     };
//
//     let consensus = Consensus::spawn(
//         ctx.clone(),
//         consensus_params,
//         params.timers_config,
//         params.gossip_consensus.clone(),
//         cal.clone(),
//         proposal_builder.clone(),
//         params.tx_decision,
//         None,
//     )
//     .await?;
//
//     let node = Node::new(
//         ctx,
//         cal,
//         params.gossip_consensus,
//         consensus,
//         params.gossip_mempool,
//         params.mempool,
//         proposal_builder,
//         params.start_height,
//     );
//
//     let actor = node.spawn().await?;
//     Ok(actor)
// }

#[allow(dead_code)]
pub struct Node<Ctx: Context> {
    ctx: Ctx,
    gossip: ActorRef<GossipMsg>,
    consensus: ActorRef<ConsensusMsg<Ctx>>,
    gossip_mempool: ActorRef<GossipMempoolMsg>,
    mempool: ActorRef<MempoolMsg>,
    host: ActorRef<HostMsg<Ctx>>,
    start_height: Ctx::Height,
}

impl<Ctx> Node<Ctx>
where
    Ctx: Context,
    Ctx::Vote: Protobuf<Proto = malachite_proto::Vote>,
    Ctx::Proposal: Protobuf<Proto = malachite_proto::Proposal>,
{
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        ctx: Ctx,
        gossip: ActorRef<GossipMsg>,
        consensus: ActorRef<ConsensusMsg<Ctx>>,
        gossip_mempool: ActorRef<GossipMempoolMsg>,
        mempool: ActorRef<MempoolMsg>,
        host: ActorRef<HostMsg<Ctx>>,
        start_height: Ctx::Height,
    ) -> Self {
        Self {
            ctx,
            gossip,
            consensus,
            gossip_mempool,
            mempool,
            host,
            start_height,
        }
    }

    pub async fn spawn(self) -> Result<(ActorRef<Msg>, JoinHandle<()>), ractor::SpawnErr> {
        Actor::spawn(None, self, ()).await
    }
}

pub enum Msg {
    Start,
}

#[async_trait]
impl<Ctx> Actor for Node<Ctx>
where
    Ctx: Context,
    Ctx::Vote: Protobuf<Proto = malachite_proto::Vote>,
    Ctx::Proposal: Protobuf<Proto = malachite_proto::Proposal>,
{
    type Msg = Msg;
    type State = ();
    type Arguments = ();

    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        _args: (),
    ) -> Result<(), ractor::ActorProcessingErr> {
        // Set ourselves as the supervisor of the other actors
        self.gossip.link(myself.get_cell());
        self.consensus.link(myself.get_cell());
        self.gossip_mempool.link(myself.get_cell());
        self.mempool.link(myself.get_cell());
        self.host.link(myself.get_cell());

        Ok(())
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        _state: &mut (),
    ) -> Result<(), ractor::ActorProcessingErr> {
        match msg {
            Msg::Start => self.mempool.cast(MempoolMsg::Start)?,
        }

        Ok(())
    }
}
