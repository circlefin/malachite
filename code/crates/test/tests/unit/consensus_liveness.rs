use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use ractor::{Actor, ActorProcessingErr, ActorRef};
use tokio::time::timeout;

use arc_malachitebft_test::utils::validators::make_validators;
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
use malachitebft_engine::util::events::{Event, TxEvent};
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

/// WAL stub that answers the RPC-style WAL commands RestartHeight issues.
///
/// `IgnoreActor` would leave `ractor::call!(WalMsg::Reset)` hanging forever.
struct WalStub;

#[async_trait]
impl Actor for WalStub {
    type Msg = WalMsg<TestContext>;
    type State = ();
    type Arguments = ();

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        _args: (),
    ) -> Result<(), ActorProcessingErr> {
        Ok(())
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        _state: &mut (),
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            WalMsg::Reset(_, reply_to) => {
                let _ = reply_to.send(Ok(()));
            }
            WalMsg::StartedHeight(_, reply_to) => {
                let _ = reply_to.send(Ok(Vec::new()));
            }
            WalMsg::Append(_, _, reply_to) => {
                let _ = reply_to.send(Ok(()));
            }
            WalMsg::Flush(reply_to) => {
                let _ = reply_to.send(Ok(()));
            }
            WalMsg::Dump => {}
        }

        Ok(())
    }
}

/// Host stub that answers the blocking `StartedRound` call during height start.
struct HostStub;

#[async_trait]
impl Actor for HostStub {
    type Msg = HostMsg<TestContext>;
    type State = ();
    type Arguments = ();

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        _args: (),
    ) -> Result<(), ActorProcessingErr> {
        Ok(())
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        _state: &mut (),
    ) -> Result<(), ActorProcessingErr> {
        if let HostMsg::StartedRound { reply_to, .. } = msg {
            let _ = reply_to.send(Vec::new());
        }

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

/// `RestartHeight` must not be buffered while the consensus actor is outside
/// `Phase::Running`. Otherwise the message sits in `msg_buffer` forever and the
/// height never restarts.
#[tokio::test]
async fn restart_height_is_processed_while_not_running() {
    let ctx = TestContext::default();
    let network = spawn_ignore_actor::<NetworkMsg<TestContext>>().await;
    let (host, _) = Actor::spawn(None, HostStub, ()).await.expect("spawn host");
    let (wal, _) = Actor::spawn(None, WalStub, ()).await.expect("spawn wal");
    let node = spawn_ignore_actor::<NodeMsg>().await;

    let tx_event = TxEvent::new();
    let mut events = tx_event.subscribe();

    let consensus = Consensus::spawn(
        ctx,
        ConsensusParams {
            // Not a member of the validator set below, so StartHeight does not
            // require a signer and does not attempt to propose.
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
        tx_event,
        node.clone(),
        tracing::Span::current(),
    )
    .await
    .expect("spawn consensus");

    let [(validator, _)] = make_validators([1]);
    let params = HeightParams::<TestContext>::new(
        ValidatorSet::new([validator]),
        LinearTimeouts::default(),
        None,
    );
    let height = Height::new(1);

    consensus
        .cast(ConsensusMsg::RestartHeight(height, params))
        .expect("cast RestartHeight");

    let started = timeout(Duration::from_secs(2), async {
        loop {
            match events.recv().await.expect("event channel open") {
                Event::StartedHeight(h, is_restart) if h == height && is_restart => break,
                _ => continue,
            }
        }
    })
    .await;

    assert!(
        started.is_ok(),
        "RestartHeight must be processed while phase != Running; \
         without the should_buffer exemption this times out"
    );

    network.stop(None);
    host.stop(None);
    wal.stop(None);
    node.stop(None);
}
