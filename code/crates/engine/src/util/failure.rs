//! Failure-handling helpers, matched to the Node supervisor's safety/liveness
//! classification of `SupervisionEvent`s:
//!
//! - [`stop_on_failure`] — **liveness path**, for failures that are safe to
//!   auto-restart (transient I/O, startup-path WAL errors, peer-task crash).
//! - [`hang_on_safety_failure`] — **safety path**, for failures where a restart
//!   is itself dangerous (WAL worker panic, runtime WAL write/flush failure —
//!   auto-restart could double-sign).

use std::future::{pending, Future};

use ractor::ActorProcessingErr;
use tracing::error;

use crate::node::{NodeMsg, NodeRef};

/// Liveness-path failure handler: on error, log the reason and return `Err` so
/// the caller `?`-propagates out of its handler.
///
/// Returning `Err` from a ractor handler terminates the actor as `ActorFailed`
/// on the spot; the Node supervisor's failure branch then stops the Node so the
/// orchestrator can restart the process. We deliberately do not call
/// `myself.stop()` — the `Err` return fails the actor on the current loop
/// iteration, before any queued stop would be polled, so a stop here is dead
/// code.
pub async fn stop_on_failure<A, E>(
    f: impl Future<Output = Result<A, E>>,
    on_error: impl FnOnce(E) -> String,
) -> Result<A, ActorProcessingErr> {
    match f.await {
        Ok(value) => Ok(value),
        Err(e) => {
            let reason = on_error(e);
            error!(reason = %reason, "Liveness failure; failing actor to trigger restart");
            Err(eyre::eyre!("liveness failure: {reason}").into())
        }
    }
}

/// Safety-path failure handler: on error, log the reason, cast
/// [`NodeMsg::SafetyFailure`] to the Node (one-way, idempotent), and hang the
/// current task. The Node stops its children and stays alive — the process is
/// held for operator inspection, no further signing occurs, and the
/// `node_safety_failure` metric flips for alerting.
pub async fn hang_on_safety_failure<A, E>(
    node: &NodeRef,
    f: impl Future<Output = Result<A, E>>,
    on_error: impl FnOnce(E) -> String,
) -> A
where
    E: std::fmt::Display,
{
    match f.await {
        Ok(value) => value,
        Err(e) => {
            let reason = on_error(e);
            error!(reason = %reason, "Safety-critical failure; hanging to prevent unsafe auto-restart");
            // One-way, idempotent; if the Node is gone the cast fails and we
            // still hang — the point is to make no further progress.
            let _ = node.cast(NodeMsg::SafetyFailure(reason));
            pending::<()>().await;
            unreachable!("safety-hang future should never resume")
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use async_trait::async_trait;
    use ractor::{Actor, ActorProcessingErr, ActorRef};
    use tokio::sync::Mutex;

    use super::*;
    use crate::node::{NodeMsg, NodeRef};

    /// Minimal test actor for [`NodeMsg`] that records every message it
    /// receives. Stands in for the real Node actor in helper-level unit tests.
    struct RecordingNode {
        received: Arc<Mutex<Vec<NodeMsg>>>,
    }

    #[async_trait]
    impl Actor for RecordingNode {
        type Msg = NodeMsg;
        type State = ();
        type Arguments = ();

        async fn pre_start(
            &self,
            _myself: NodeRef,
            _args: (),
        ) -> Result<Self::State, ActorProcessingErr> {
            Ok(())
        }

        async fn handle(
            &self,
            _myself: NodeRef,
            msg: Self::Msg,
            _state: &mut Self::State,
        ) -> Result<(), ActorProcessingErr> {
            self.received.lock().await.push(msg);
            Ok(())
        }
    }

    async fn spawn_recording_node() -> (NodeRef, Arc<Mutex<Vec<NodeMsg>>>) {
        let received = Arc::new(Mutex::new(Vec::new()));
        let (node_ref, _) = Actor::spawn(
            None,
            RecordingNode {
                received: received.clone(),
            },
            (),
        )
        .await
        .expect("spawn RecordingNode");
        (node_ref, received)
    }

    /// Small pause to let ractor deliver a cast message.
    async fn yield_a_bit() {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    #[tokio::test]
    async fn hang_on_safety_failure_casts_safety_failure_and_hangs() {
        let (node, received) = spawn_recording_node().await;

        // Run the helper in a task so we can verify it never completes.
        let task = tokio::spawn({
            let node = node.clone();
            async move {
                let fut = async { Err::<(), &'static str>("disk full") };
                let _: () =
                    hang_on_safety_failure(&node, fut, |e| format!("wal_append: {e}")).await;
            }
        });

        yield_a_bit().await;

        // The helper must have signalled the Node before hanging.
        let msgs = received.lock().await;
        assert_eq!(msgs.len(), 1, "expected exactly one NodeMsg cast");
        match &msgs[0] {
            NodeMsg::SafetyFailure(reason) => {
                assert!(reason.contains("wal_append"), "reason was: {reason}");
                assert!(reason.contains("disk full"), "reason was: {reason}");
            }
        }
        drop(msgs);

        // And the helper must still be hanging — the whole point is to not
        // make progress past an un-recorded signing.
        assert!(
            !task.is_finished(),
            "hang_on_safety_failure should hang on error, not return"
        );

        task.abort();
    }

    #[tokio::test]
    async fn hang_on_safety_failure_returns_success_value_unchanged() {
        let (node, received) = spawn_recording_node().await;

        let value: u32 =
            hang_on_safety_failure(&node, async { Ok::<u32, &'static str>(42) }, |_| {
                "should not be called".to_string()
            })
            .await;

        assert_eq!(value, 42);
        assert!(
            received.lock().await.is_empty(),
            "success path must not cast SafetyFailure"
        );
    }

    #[tokio::test]
    async fn stop_on_failure_returns_err_on_failure() {
        // On error the helper returns `Err` so the caller can `?`-propagate,
        // which fails (and thus terminates) the actor.
        let fut = async { Err::<(), &'static str>("transient io") };
        let result = stop_on_failure(fut, |e| format!("wal_fetch: {e}")).await;

        assert!(
            result.is_err(),
            "stop_on_failure must return Err so the caller can ?-propagate"
        );
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("wal_fetch"), "error was: {err_msg}");
        assert!(err_msg.contains("transient io"), "error was: {err_msg}");
    }

    #[tokio::test]
    async fn stop_on_failure_returns_success_value_unchanged() {
        let value: u32 = stop_on_failure(async { Ok::<u32, &'static str>(7) }, |_| {
            "should not be called".to_string()
        })
        .await
        .expect("success path must return Ok");

        assert_eq!(value, 7);
    }

    /// Regression: the helper must terminate the calling actor when invoked
    /// from inside that actor's own `handle`. It does so by returning `Err`,
    /// which fails the actor as soon as the handler unwinds. This reproduces
    /// the Consensus actor's call shape.
    #[tokio::test]
    async fn stop_on_failure_from_inside_handle_terminates_the_actor() {
        /// Actor whose `handle` invokes `stop_on_failure`, simulating the
        /// Consensus liveness path.
        struct StopOnFailureActor;

        #[async_trait]
        impl Actor for StopOnFailureActor {
            type Msg = ();
            type State = ();
            type Arguments = ();

            async fn pre_start(
                &self,
                _myself: ActorRef<()>,
                _args: (),
            ) -> Result<Self::State, ActorProcessingErr> {
                Ok(())
            }

            async fn handle(
                &self,
                _myself: ActorRef<()>,
                _msg: (),
                _state: &mut Self::State,
            ) -> Result<(), ActorProcessingErr> {
                let fut = async { Err::<(), &'static str>("boom") };
                stop_on_failure(fut, |e| format!("liveness test: {e}")).await?;
                Ok(())
            }
        }

        let (actor, _join) = Actor::spawn(None, StopOnFailureActor, ())
            .await
            .expect("spawn StopOnFailureActor");

        // Trigger the handler path that invokes the helper.
        actor.cast(()).expect("cast trigger");

        // The actor must terminate: the helper returns `Err`, `handle` unwinds,
        // and ractor fails the actor on the spot.
        actor
            .wait(Some(Duration::from_secs(2)))
            .await
            .expect("actor must terminate after stop_on_failure returns Err from its own handle");
    }
}
