use ractor::message::Message;
use ractor::ActorRef;
use tracing::debug;
use std::time::Duration;

pub async fn ticker<Msg>(interval: Duration, target: ActorRef<Msg>, msg: impl Fn() -> Msg)
where
    Msg: Message,
{
    loop {
        tokio::time::sleep(interval).await;
        debug!("sending message generatetransactions");

        if let Err(er) = target.cast(msg()) {
            tracing::error!(?er, ?target, "Failed to send tick message");
            break;
        }
    }
}
