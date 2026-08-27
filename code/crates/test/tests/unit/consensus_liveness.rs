use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use ractor::{Actor, ActorProcessingErr, ActorRef};

use arc_malachitebft_test::{
    Address, Ed25519Verifier, Height, LinearTimeouts, TestContext, ValidatorSet,
};

use malachitebft_config::ConsensusConfig;
use malachitebft_core_types::{HeightParams, ThresholdParams, ValuePayload};
use malachitebft_engine::consensus::{Consensus, ConsensusMsg, ConsensusParams};
use malachitebft_engine::host::HostMsg;
use malachitebft_engine::network::NetworkMsg;
use malachitebft_engine::node::NodeMsg;
use malachitebft_engine::sync::Msg as SyncMsg;
use malachitebft_engine::util::events::TxEvent;
use malachitebft_engine::util::output_port::OutputPort;
use malachitebft_engine::wal::Msg as WalMsg;
use malachitebft_metrics::{Metrics, SharedRegistry};

struct IgnoreActor<Msg> {
    _marker: PhantomData<fn(Msg)>,
}

#[async_trait]
impl<Msg> Actor for IgnoreActor<Msg>
where
    Msg: ractor::Message,
{
    type Msg = Msg;
    type State = ();
    type Arguments = ();

    async fn pre_start(&self, _myself: ActorRef<Msg>, _args: ()) -> Result<(), ActorProcessingErr> {
        Ok(())
    }

    async fn handle(
        &self,
        _myself: ActorRef<Msg>,
        _msg: Msg,
        _state: &mut (),
    ) -> Result<(), ActorProcessingErr> {
        Ok(())
    }
}

async fn spawn_ignore_actor<Msg>() -> ActorRef<Msg>
where
    Msg: ractor::Message,
{
    let (actor, _) = Actor::spawn(
        None,
        IgnoreActor {
            _marker: PhantomData,
        },
        (),
    )
    .await
    .expect("spawn ignore actor");

    actor
}

fn metrics_in_isolated_registry() -> Metrics {
    let registry = SharedRegistry::new(Default::default(), None);
    Metrics::register(&registry)
}

#[tokio::test]
async fn consensus_actor_terminates_on_start_height_error() {
    let ctx = TestContext::default();
    let network = spawn_ignore_actor::<NetworkMsg<TestContext>>().await;
    let host = spawn_ignore_actor::<HostMsg<TestContext>>().await;
    let wal = spawn_ignore_actor::<WalMsg<TestContext>>().await;
    let node = spawn_ignore_actor::<NodeMsg>().await;

    let consensus = Consensus::spawn(
        ctx,
        ConsensusParams {
            address: Address::new([0; 20]),
            threshold_params: ThresholdParams::default(),
            value_payload: ValuePayload::ProposalAndParts,
            enabled: true,
        },
        ConsensusConfig::default(),
        Box::new(Ed25519Verifier),
        None,
        network.clone(),
        host.clone(),
        wal.clone(),
        Arc::new(OutputPort::<SyncMsg<TestContext>>::new()),
        metrics_in_isolated_registry(),
        TxEvent::new(),
        node.clone(),
        tracing::Span::current(),
    )
    .await
    .expect("spawn consensus");

    let empty_validator_set = ValidatorSet {
        validators: Arc::new(Vec::new()),
    };
    let params =
        HeightParams::<TestContext>::new(empty_validator_set, LinearTimeouts::default(), None);

    consensus
        .cast(ConsensusMsg::StartHeight(Height::new(1), params))
        .expect("cast StartHeight");

    consensus
        .wait(Some(Duration::from_secs(2)))
        .await
        .expect("consensus actor should terminate on StartHeight error");

    network.stop(None);
    host.stop(None);
    wal.stop(None);
    node.stop(None);
}
