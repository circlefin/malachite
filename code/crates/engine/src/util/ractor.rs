use ractor::{ActorRef, Message, MessagingErr, RpcReplyPort};

/// Send a message with an `RpcReplyPort<TReply>` to `target` and spawn a task
/// that handles the response with `on_reply`. Channel errors (target actor
/// died) are logged.
pub fn cast_and_handle<TMsg, TReply>(
    target: &ActorRef<TMsg>,
    msg_factory: impl FnOnce(RpcReplyPort<TReply>) -> TMsg,
    on_reply: impl FnOnce(TReply) + Send + 'static,
) -> Result<(), MessagingErr<TMsg>>
where
    TMsg: Message,
    TReply: Send + 'static,
{
    let (tx, rx) = ractor::concurrency::oneshot();
    target.cast(msg_factory(tx.into()))?;

    ractor::concurrency::spawn(async move {
        match rx.await {
            Ok(reply) => on_reply(reply),
            Err(_) => {
                tracing::error!("Actor dropped reply channel");
            }
        }
    });

    Ok(())
}
