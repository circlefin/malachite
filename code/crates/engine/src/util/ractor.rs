use ractor::{ActorRef, Message, MessagingErr, RpcReplyPort};

/// Send a message with an `RpcReplyPort<Option<TReply>>` to `target` and spawn a task
/// that handles the response: `on_some` when the reply is `Some`, `on_none` when `None`.
/// Channel errors (target actor died) are logged.
pub fn cast_option_and_handle<TMsg, TReply>(
    target: &ActorRef<TMsg>,
    msg_factory: impl FnOnce(RpcReplyPort<Option<TReply>>) -> TMsg,
    on_some: impl FnOnce(TReply) + Send + 'static,
    on_none: impl FnOnce() + Send + 'static,
) -> Result<(), MessagingErr<TMsg>>
where
    TMsg: Message,
    TReply: Send + 'static,
{
    let (tx, rx) = ractor::concurrency::oneshot();
    target.cast(msg_factory(tx.into()))?;

    ractor::concurrency::spawn(async move {
        match rx.await {
            Ok(Some(value)) => on_some(value),
            Ok(None) => on_none(),
            Err(_) => {
                tracing::error!("Actor dropped reply channel");
            }
        }
    });

    Ok(())
}
