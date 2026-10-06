use ractor::{ActorRef, Message, MessagingErr, RpcReplyPort};

/// The target actor dropped the `RpcReplyPort` without sending a value
/// (typically actor death or restart).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReplyDropped;

/// Send a message with an `RpcReplyPort<TReply>` to `target` and spawn a task
/// that handles the oneshot result with `on_reply`. A dropped reply port
/// (actor death or restart) is delivered as `Err(ReplyDropped)` so the caller
/// can release any slot that was waiting on that reply.
pub fn cast_and_handle<TMsg, TReply>(
    target: &ActorRef<TMsg>,
    msg_factory: impl FnOnce(RpcReplyPort<TReply>) -> TMsg,
    on_reply: impl FnOnce(Result<TReply, ReplyDropped>) + Send + 'static,
) -> Result<(), MessagingErr<TMsg>>
where
    TMsg: Message,
    TReply: Send + 'static,
{
    let (tx, rx) = ractor::concurrency::oneshot();
    target.cast(msg_factory(tx.into()))?;

    ractor::concurrency::spawn(async move {
        match rx.await {
            Ok(reply) => on_reply(Ok(reply)),
            Err(_) => {
                tracing::error!("Actor dropped reply channel");
                on_reply(Err(ReplyDropped));
            }
        }
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    use async_trait::async_trait;
    use ractor::{Actor, ActorProcessingErr, ActorRef};

    use super::*;

    enum TestMsg {
        Ask(RpcReplyPort<u8>),
    }

    struct DroppingHost;

    #[async_trait]
    impl Actor for DroppingHost {
        type Msg = TestMsg;
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
            _myself: ActorRef<Self::Msg>,
            _msg: Self::Msg,
            _state: &mut Self::State,
        ) -> Result<(), ActorProcessingErr> {
            Ok(())
        }
    }

    struct ReplyingHost;

    #[async_trait]
    impl Actor for ReplyingHost {
        type Msg = TestMsg;
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
            _myself: ActorRef<Self::Msg>,
            msg: Self::Msg,
            _state: &mut Self::State,
        ) -> Result<(), ActorProcessingErr> {
            match msg {
                TestMsg::Ask(reply_to) => {
                    reply_to.send(7)?;
                }
            }
            Ok(())
        }
    }

    async fn yield_a_bit() {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    #[tokio::test]
    async fn dropped_reply_is_delivered_as_err() {
        let (host, _) = Actor::spawn(None, DroppingHost, ())
            .await
            .expect("spawn DroppingHost");

        let replied = Arc::new(AtomicBool::new(false));
        let dropped = Arc::new(AtomicBool::new(false));
        let replied_flag = Arc::clone(&replied);
        let dropped_flag = Arc::clone(&dropped);

        cast_and_handle(&host, TestMsg::Ask, move |result| match result {
            Ok(_) => replied_flag.store(true, Ordering::SeqCst),
            Err(ReplyDropped) => dropped_flag.store(true, Ordering::SeqCst),
        })
        .expect("cast");

        yield_a_bit().await;

        assert!(
            !replied.load(Ordering::SeqCst),
            "dropped reply must not be delivered as Ok"
        );
        assert!(
            dropped.load(Ordering::SeqCst),
            "dropped reply must be delivered as Err(ReplyDropped)"
        );
    }

    #[tokio::test]
    async fn successful_reply_is_delivered_as_ok() {
        let (host, _) = Actor::spawn(None, ReplyingHost, ())
            .await
            .expect("spawn ReplyingHost");

        let replied = Arc::new(AtomicBool::new(false));
        let dropped = Arc::new(AtomicBool::new(false));
        let replied_flag = Arc::clone(&replied);
        let dropped_flag = Arc::clone(&dropped);

        cast_and_handle(&host, TestMsg::Ask, move |result| match result {
            Ok(value) => {
                assert_eq!(value, 7);
                replied_flag.store(true, Ordering::SeqCst);
            }
            Err(ReplyDropped) => dropped_flag.store(true, Ordering::SeqCst),
        })
        .expect("cast");

        yield_a_bit().await;

        assert!(
            replied.load(Ordering::SeqCst),
            "successful reply must be delivered as Ok"
        );
        assert!(
            !dropped.load(Ordering::SeqCst),
            "successful reply must not be delivered as Err"
        );
    }
}
