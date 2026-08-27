use async_trait::async_trait;
use ractor::{Actor, ActorProcessingErr, ActorRef, SupervisionEvent};
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

use malachitebft_metrics::Metrics;

/// Messages handled by the [`Node`] supervisor.
///
/// Kept non-generic so [`NodeRef`] can be passed to components that do not
/// otherwise carry a `Ctx` parameter (e.g. the WAL worker thread).
#[derive(Debug)]
pub enum NodeMsg {
    /// A safety-critical failure (e.g. WAL write failure, WAL worker panic).
    /// The Node stops all children and hangs — auto-restart would risk
    /// double-signing. The payload is a human-readable reason for logs.
    ///
    /// NB: this arrives as a message, which ractor polls at lower priority
    /// than child-termination supervision events. That is safe only because a
    /// component signalling a safety failure never also self-terminates: the
    /// WAL actor swallows worker errors and stays alive, and Consensus hangs
    /// rather than fails — so no liveness event can preempt this one.
    SafetyFailure(String),
}

pub type NodeRef = ActorRef<NodeMsg>;

pub struct Node {
    metrics: Metrics,
    span: tracing::Span,
}

pub struct NodeState {
    /// When `true`, the Node is in the safety-hang state: children are being
    /// stopped, the Node stays alive for operator investigation, and further
    /// supervisor events are logged but do not trigger an exit.
    safety_hang: bool,
    /// When `true`, the Node is already stopping, so child failures are part of
    /// teardown and should not re-enter the liveness restart path.
    shutting_down: bool,
}

impl Node {
    pub fn new(metrics: Metrics, span: tracing::Span) -> Self {
        Self { metrics, span }
    }

    pub async fn spawn(self) -> Result<(NodeRef, JoinHandle<()>), ractor::SpawnErr> {
        Actor::spawn(None, self, ()).await
    }
}

#[async_trait]
impl Actor for Node {
    type Msg = NodeMsg;
    type State = NodeState;
    type Arguments = ();

    async fn pre_start(
        &self,
        _myself: NodeRef,
        _args: (),
    ) -> Result<Self::State, ActorProcessingErr> {
        // Child actors link themselves to the Node via `child.link(node.get_cell())`
        // after the Node is spawned, so there is no per-child wiring here.
        Ok(NodeState {
            safety_hang: false,
            shutting_down: false,
        })
    }

    #[tracing::instrument(name = "node", parent = &self.span, skip_all)]
    async fn handle(
        &self,
        myself: NodeRef,
        msg: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            NodeMsg::SafetyFailure(reason) => {
                if state.safety_hang {
                    // Already hanging; log the extra reason but don't re-stop children.
                    warn!(
                        reason = %reason,
                        "Additional safety failure reported while already in safety-hang state"
                    );
                    return Ok(());
                }

                error!(reason = %reason, "Safety-critical failure detected");
                error!(
                    "Entering safety-hang state: stopping all children, \
                     Node remains alive — operator intervention required before restart"
                );

                state.safety_hang = true;
                self.metrics.set_safety_failure();

                // Stop children but not the Node, so the process stays alive for
                // inspection. The resulting supervisor events are absorbed by the
                // `safety_hang` branch in `handle_supervisor_evt`.
                myself.get_cell().stop_children(Some(reason));

                Ok(())
            }
        }
    }

    #[tracing::instrument(name = "node", parent = &self.span, skip_all)]
    async fn handle_supervisor_evt(
        &self,
        myself: NodeRef,
        evt: SupervisionEvent,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match evt {
            SupervisionEvent::ActorStarted(cell) => {
                info!(actor = %cell.get_id(), "Actor has started");
                Ok(())
            }

            SupervisionEvent::ActorTerminated(cell, _state, reason) => {
                let reason_str = reason.unwrap_or_default();

                if state.safety_hang {
                    // Children we asked to stop during safety-hang: log and stay.
                    info!(
                        actor = %cell.get_id(), reason = %reason_str,
                        "Actor terminated during safety-hang (expected)"
                    );
                    return Ok(());
                }

                if state.shutting_down {
                    info!(
                        actor = %cell.get_id(), reason = %reason_str,
                        "Actor terminated during Node shutdown (expected)"
                    );
                    return Ok(());
                }

                warn!(
                    actor = %cell.get_id(), reason = %reason_str,
                    "Actor terminated — stopping Node to trigger orchestrator restart"
                );

                // Liveness path: stop the Node so the orchestrator can restart
                // the process cleanly.
                state.shutting_down = true;

                myself.stop(Some(format!(
                    "child actor terminated: {} ({reason_str})",
                    cell.get_id()
                )));

                Ok(())
            }

            SupervisionEvent::ActorFailed(cell, error) => {
                if state.safety_hang {
                    info!(
                        actor = %cell.get_id(), error = %error,
                        "Actor failed during safety-hang (expected)"
                    );
                    return Ok(());
                }

                if state.shutting_down {
                    info!(
                        actor = %cell.get_id(), error = %error,
                        "Actor failed during Node shutdown (expected)"
                    );
                    return Ok(());
                }

                error!(
                    actor = %cell.get_id(), error = %error,
                    "Actor failed — stopping Node to trigger orchestrator restart"
                );

                state.shutting_down = true;

                myself.stop(Some(format!(
                    "child actor failed: {} ({error})",
                    cell.get_id()
                )));

                Ok(())
            }

            SupervisionEvent::ProcessGroupChanged(_) => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use malachitebft_metrics::SharedRegistry;
    use ractor::concurrency::sleep;
    use ractor::{Actor, ActorProcessingErr, ActorRef};

    use super::*;

    /// Minimal child actor used to exercise the Node's supervisor policy.
    ///
    /// `Msg::Panic`  — return an error from `handle` so the actor transitions
    /// to `ActorFailed`.
    /// `Msg::Stop`   — call `myself.stop(...)`, driving an `ActorTerminated`
    /// supervisor event.
    struct Child;

    #[derive(Debug)]
    enum ChildMsg {
        Panic(String),
        Stop(String),
    }

    #[async_trait]
    impl Actor for Child {
        type Msg = ChildMsg;
        type State = ();
        type Arguments = ();

        async fn pre_start(
            &self,
            _myself: ActorRef<Self::Msg>,
            _args: (),
        ) -> Result<Self::State, ActorProcessingErr> {
            Ok(())
        }

        async fn handle(
            &self,
            myself: ActorRef<Self::Msg>,
            msg: Self::Msg,
            _state: &mut Self::State,
        ) -> Result<(), ActorProcessingErr> {
            match msg {
                ChildMsg::Panic(reason) => Err(eyre::eyre!("{reason}").into()),
                ChildMsg::Stop(reason) => {
                    myself.stop(Some(reason));
                    Ok(())
                }
            }
        }
    }

    async fn spawn_node_with_child() -> (NodeRef, ActorRef<ChildMsg>, Metrics) {
        // Use an isolated registry per test so the gauge starts at 0.
        let registry = SharedRegistry::new(Default::default(), None);
        let metrics = Metrics::register(&registry);

        let node = Node::new(metrics.clone(), tracing::Span::current());
        let (node_ref, _) = node.spawn().await.expect("spawn Node");

        let (child_ref, _) = Actor::spawn(None, Child, ()).await.expect("spawn Child");
        child_ref.link(node_ref.get_cell());

        (node_ref, child_ref, metrics)
    }

    #[tokio::test]
    async fn child_failure_stops_node_and_leaves_metric_zero() {
        let (node, child, metrics) = spawn_node_with_child().await;

        // Force the child to fail; supervisor should treat this as a liveness
        // failure and stop the Node so the orchestrator can restart.
        child
            .cast(ChildMsg::Panic("liveness test".into()))
            .expect("cast Panic");

        node.wait(Some(Duration::from_secs(2)))
            .await
            .expect("Node should stop within the timeout");

        // Liveness path must NOT set the safety_failure gauge.
        assert_eq!(
            metrics.node_safety_failure.get(),
            0,
            "liveness failure must not flip node_safety_failure"
        );
    }

    #[tokio::test]
    async fn child_termination_stops_node() {
        let (node, child, _metrics) = spawn_node_with_child().await;

        child
            .cast(ChildMsg::Stop("clean shutdown of child".into()))
            .expect("cast Stop");

        node.wait(Some(Duration::from_secs(2)))
            .await
            .expect("Node should stop after any unexpected child termination");
    }

    #[tokio::test]
    async fn child_failure_during_node_shutdown_does_not_stop_node_again() {
        let registry = SharedRegistry::new(Default::default(), None);
        let metrics = Metrics::register(&registry);
        let node_actor = Node::new(metrics.clone(), tracing::Span::current());
        let (node_ref, _) = Node::new(metrics.clone(), tracing::Span::current())
            .spawn()
            .await
            .expect("spawn Node");
        let (child_ref, _) = Actor::spawn(None, Child, ()).await.expect("spawn Child");

        let mut state = NodeState {
            safety_hang: false,
            shutting_down: true,
        };

        node_actor
            .handle_supervisor_evt(
                node_ref.clone(),
                SupervisionEvent::ActorFailed(
                    child_ref.get_cell(),
                    eyre::eyre!("late shutdown failure").into(),
                ),
                &mut state,
            )
            .await
            .expect("handle supervisor event");

        match node_ref.wait(Some(Duration::from_millis(100))).await {
            Err(_) => {}
            Ok(()) => panic!("Node must stay alive after child failure during shutdown"),
        }

        assert_eq!(
            metrics.node_safety_failure.get(),
            0,
            "shutdown child failure must not flip node_safety_failure"
        );

        child_ref.stop(None);
        node_ref.stop(None);
    }

    #[tokio::test]
    async fn safety_failure_hangs_node_stops_children_and_sets_metric() {
        let (node, child, metrics) = spawn_node_with_child().await;

        node.cast(NodeMsg::SafetyFailure("test WAL failure".into()))
            .expect("cast SafetyFailure");

        // The Node should stop the child as part of entering safety-hang.
        child
            .wait(Some(Duration::from_secs(2)))
            .await
            .expect("child should be stopped by Node during safety-hang");

        // Metric flipped.
        assert_eq!(
            metrics.node_safety_failure.get(),
            1,
            "node_safety_failure gauge must be set on safety-hang"
        );

        // And the Node itself must still be alive — that's the whole point.
        // Wait briefly; `wait` should time out.
        match node.wait(Some(Duration::from_millis(200))).await {
            Err(_) => { /* timeout = still alive, as expected */ }
            Ok(()) => panic!("Node must NOT stop on SafetyFailure"),
        }

        // Cleanup.
        node.stop(None);
    }

    #[tokio::test]
    async fn safety_failure_is_idempotent() {
        let (node, child, metrics) = spawn_node_with_child().await;

        node.cast(NodeMsg::SafetyFailure("first".into()))
            .expect("cast first");

        // A second SafetyFailure after the first must not crash or reset state.
        // Give the first message a moment to process.
        sleep(Duration::from_millis(50)).await;
        node.cast(NodeMsg::SafetyFailure("second".into()))
            .expect("cast second");

        child
            .wait(Some(Duration::from_secs(2)))
            .await
            .expect("child should be stopped");

        assert_eq!(metrics.node_safety_failure.get(), 1);

        // Node still alive after two SafetyFailures.
        match node.wait(Some(Duration::from_millis(100))).await {
            Err(_) => {}
            Ok(()) => panic!("Node must stay alive across repeated SafetyFailures"),
        }

        node.stop(None);
    }

    #[tokio::test]
    async fn child_terminating_during_safety_hang_does_not_stop_node() {
        // Regression: once Node has entered safety-hang, stopping children is
        // expected — the supervisor event path must not fall through to the
        // liveness stop-Node logic, or we'd auto-restart after all.
        let (node, _child, _metrics) = spawn_node_with_child().await;

        node.cast(NodeMsg::SafetyFailure("entering hang".into()))
            .expect("cast SafetyFailure");

        // Wait long enough for the child's ActorTerminated event to reach the
        // Node's supervisor handler.
        sleep(Duration::from_millis(200)).await;

        match node.wait(Some(Duration::from_millis(100))).await {
            Err(_) => {}
            Ok(()) => panic!(
                "Node stopped after child terminated during safety-hang; \
                 supervisor did not honour safety_hang flag"
            ),
        }

        node.stop(None);
    }
}
