use std::time::Duration;

use ractor::message::Message;
use ractor::ActorRef;

pub async fn ticker<Msg>(interval: Duration, target: ActorRef<Msg>, msg: impl Fn() -> Msg)
where
    Msg: Message,
{
    loop {
        tokio::time::sleep(interval).await;

        if let Err(e) = target.cast(msg()) {
            tracing::error!(target = %target.get_id(), "Failed to send tick message: {e}");
            break;
        }
    }
}
