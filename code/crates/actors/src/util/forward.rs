use std::marker::PhantomData;
use std::sync::atomic::AtomicPtr;

use ractor::{Actor, ActorCell, ActorRef, Message};

pub struct Forward<A, B, F> {
    to: ActorRef<B>,
    map: F,
    _marker: PhantomData<AtomicPtr<A>>,
}

#[ractor::async_trait]
impl<A, B, F> Actor for Forward<A, B, F>
where
    A: Message,
    B: Message,
    F: Fn(A) -> B + Send + Sync + 'static,
{
    type Msg = A;
    type State = ();
    type Arguments = ();

    async fn pre_start(
        &self,
        _myself: ActorRef<A>,
        _args: (),
    ) -> Result<(), ractor::ActorProcessingErr> {
        Ok(())
    }

    async fn handle(
        &self,
        _myself: ActorRef<A>,
        msg: A,
        _state: &mut (),
    ) -> Result<(), ractor::ActorProcessingErr> {
        let msg = (self.map)(msg);
        self.to.cast(msg)?;
        Ok(())
    }
}

pub async fn forward<A, B, F>(
    to: ActorRef<B>,
    supervisor: Option<ActorCell>,
    map: F,
) -> Result<ActorRef<A>, ractor::SpawnErr>
where
    A: Message,
    B: Message,
    F: Fn(A) -> B + Send + Sync + 'static,
{
    let actor = Forward {
        to,
        map: Box::new(map),
        _marker: PhantomData,
    };

    let (actor_ref, _) = if let Some(supervisor) = supervisor {
        Actor::spawn_linked(None, actor, (), supervisor).await?
    } else {
        Actor::spawn(None, actor, ()).await?
    };

    Ok(actor_ref)
}
