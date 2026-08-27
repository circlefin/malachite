//! Tests for the `Node` supervisor's safety/liveness policy against the real
//! `Wal` actor. They wire up only the minimum a Node needs to observe a child
//! failure (plus a real WAL for the thread-panic case), so they run in the
//! regular unit-test profile without the integration framework.

use std::time::Duration;

use bytes::Bytes;
use ractor::{Actor, ActorRef};
use tempfile::TempDir;
use tokio::sync::oneshot;

use arc_malachitebft_test::codec::proto::ProtobufCodec;
use arc_malachitebft_test::{Address, Height, Signature, TestContext, Value, Vote};

use malachitebft_codec::Codec;
use malachitebft_core_consensus::Input;
use malachitebft_core_types::{NilOrVal, Round, SignedVote};
use malachitebft_engine::node::{Node, NodeRef};
use malachitebft_engine::wal::{Msg as WalMsg, Wal, WalRef};
use malachitebft_metrics::{Metrics, SharedRegistry};

/// A codec that decodes via [`ProtobufCodec`] but panics on every encode.
/// Asking the WAL to persist any codec-encoded entry (e.g. a vote) therefore
/// panics the worker thread — the failure this test exercises — independently
/// of which entry types the WAL currently chooses to persist.
#[derive(Copy, Clone, Debug)]
struct PanickingCodec;

impl<T> Codec<T> for PanickingCodec
where
    ProtobufCodec: Codec<T>,
{
    type Error = <ProtobufCodec as Codec<T>>::Error;

    fn decode(&self, bytes: Bytes) -> Result<T, Self::Error> {
        ProtobufCodec.decode(bytes)
    }

    fn encode(&self, _msg: &T) -> Result<Bytes, Self::Error> {
        panic!("PanickingCodec: deliberate WAL encode panic");
    }
}

fn metrics_in_isolated_registry() -> Metrics {
    let registry = SharedRegistry::new(Default::default(), None);
    Metrics::register(&registry)
}

async fn spawn_node() -> (NodeRef, Metrics) {
    let metrics = metrics_in_isolated_registry();
    let node = Node::new(metrics.clone(), tracing::Span::current());
    let (node_ref, _) = node.spawn().await.expect("spawn Node");
    (node_ref, metrics)
}

async fn spawn_wal(node: NodeRef, path: std::path::PathBuf) -> WalRef<TestContext> {
    let wal = Wal::<TestContext, PanickingCodec>::spawn(
        &TestContext::default(),
        PanickingCodec,
        path,
        SharedRegistry::new(Default::default(), None),
        tracing::Span::current(),
        node,
    )
    .await
    .expect("spawn Wal");
    wal
}

async fn wait_gauge(metrics: &Metrics, target: i64, timeout: Duration) -> bool {
    let deadline = tokio::time::Instant::now() + timeout;
    while tokio::time::Instant::now() < deadline {
        if metrics.node_safety_failure.get() == target {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    metrics.node_safety_failure.get() == target
}

/// A panic in the WAL worker thread must drive the Node into safety-hang. We
/// force the panic with a codec that panics on encode (see `PanickingCodec`),
/// then ask the WAL to persist a vote — which routes through the codec. The
/// Node must then enter safety-hang: `node_safety_failure` at 1, children
/// stopped, Node itself still alive.
#[tokio::test]
async fn wal_worker_thread_panic_triggers_node_safety_hang() {
    let (node, metrics) = spawn_node().await;

    let tmp = TempDir::new().expect("tempdir");
    let wal_path = tmp.path().join("wal");

    let wal: WalRef<TestContext> = spawn_wal(node.clone(), wal_path).await;
    wal.link(node.get_cell());

    // Drive the WAL to a specific height so Append is not rejected as
    // mismatched-height. `StartedHeight` returns any previously-persisted
    // entries; we don't care about them.
    let _started: Vec<_> = ractor::call!(wal, WalMsg::StartedHeight, Height::new(1))
        .expect("StartedHeight call")
        .expect("StartedHeight reply");

    // Append a vote: persisting it routes through the codec, which panics. The
    // worker catches the panic and casts `NodeMsg::SafetyFailure` before exiting.
    // We don't await the reply — the panic drops the reply channel.
    let (tx, _rx) = oneshot::channel();
    let vote = Input::Vote(SignedVote::new(
        Vote::new_prevote(
            Height::new(1),
            Round::new(0),
            NilOrVal::Val(Value::new(100).id()),
            Address::new([0; 20]),
        ),
        Signature::test(),
    ));
    let append = WalMsg::Append(Height::new(1), vote, ractor::RpcReplyPort::from(tx));
    wal.cast(append).expect("cast Append");

    assert!(
        wait_gauge(&metrics, 1, Duration::from_secs(5)).await,
        "node_safety_failure should flip to 1 after WAL worker panic"
    );

    // The Node itself must remain alive. Give the supervisor a moment to process
    // the WAL actor's termination, then check it's still up.
    tokio::time::sleep(Duration::from_millis(200)).await;
    match node.wait(Some(Duration::from_millis(200))).await {
        Err(_) => { /* timeout = still alive, as expected */ }
        Ok(()) => panic!(
            "Node stopped after WAL thread panic; safety-hang must keep the \
             Node alive for operator inspection"
        ),
    }

    node.stop(None);
}

/// Regression: a non-WAL child failing (we simulate with a dummy actor) must
/// be treated as a liveness event — the Node stops so the orchestrator can
/// restart the process, and the safety gauge does NOT flip.
#[tokio::test]
async fn non_wal_child_failure_stops_node_without_touching_safety_gauge() {
    use ractor::{ActorProcessingErr, SupervisionEvent};

    struct FailingChild;
    #[derive(Debug)]
    struct Crash;

    #[async_trait::async_trait]
    impl Actor for FailingChild {
        type Msg = Crash;
        type State = ();
        type Arguments = ();

        async fn pre_start(
            &self,
            _myself: ActorRef<Crash>,
            _args: (),
        ) -> Result<(), ActorProcessingErr> {
            Ok(())
        }

        async fn handle(
            &self,
            _myself: ActorRef<Crash>,
            _msg: Crash,
            _state: &mut (),
        ) -> Result<(), ActorProcessingErr> {
            Err(eyre::eyre!("simulated non-WAL child crash").into())
        }

        async fn handle_supervisor_evt(
            &self,
            _myself: ActorRef<Crash>,
            _evt: SupervisionEvent,
            _state: &mut (),
        ) -> Result<(), ActorProcessingErr> {
            Ok(())
        }
    }

    let (node, metrics) = spawn_node().await;
    let (child_ref, _) = Actor::spawn(None, FailingChild, ())
        .await
        .expect("spawn child");
    child_ref.link(node.get_cell());

    child_ref.cast(Crash).expect("cast Crash");

    // The supervisor should react by stopping the Node for orchestrator restart.
    node.wait(Some(Duration::from_secs(2)))
        .await
        .expect("Node should stop after non-safety child failure");

    // The safety gauge stays at 0 — a restart is safe for liveness failures.
    assert_eq!(
        metrics.node_safety_failure.get(),
        0,
        "liveness path must not flip node_safety_failure"
    );
}
