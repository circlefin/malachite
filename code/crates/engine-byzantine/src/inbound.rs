//! Receiver-side filtering of inbound `NetworkEvent`s.
//!
//! The [`ByzantineNetworkProxy`](crate::proxy::ByzantineNetworkProxy) intercepts
//! outbound traffic only. To let a single node act as if it *received* fewer
//! messages than its peers (e.g. missed a specific `SignedProposal` for a
//! given `(height, round)`), we also need a filter on the inbound path.
//!
//! [`InboundFilter`] is a tiny ractor actor that acts as a man-in-the-middle
//! between the real network's `OutputPort<NetworkEvent<Ctx>>` and the
//! downstream subscriber (consensus in practice). It is installed when the
//! proxy intercepts a `NetworkMsg::Subscribe(inner)` call and the owning
//! [`ByzantineConfig`](crate::config::ByzantineConfig) has
//! [`drop_inbound_proposals`](crate::config::ByzantineConfig::drop_inbound_proposals)
//! set. Callers subscribe the forwarder to the real network in place of the
//! original subscriber; the forwarder then drops or delegates each event.

use async_trait::async_trait;
use ractor::{Actor, ActorProcessingErr, ActorRef};
use rand::rngs::StdRng;
use tracing::warn;

use malachitebft_core_types::{Context, Proposal};
use malachitebft_engine::network::{NetworkEvent, Subscriber};

use crate::config::{make_rng, Trigger};

/// Message type for [`InboundFilter`].
///
/// Implements `From<NetworkEvent<Ctx>>` so that the forwarder's `ActorRef`
/// satisfies `Subscriber<NetworkEvent<Ctx>>` via the blanket impl in the
/// engine crate.
pub enum InboundFilterMsg<Ctx: Context> {
    Event(NetworkEvent<Ctx>),
}

impl<Ctx: Context> From<NetworkEvent<Ctx>> for InboundFilterMsg<Ctx> {
    fn from(event: NetworkEvent<Ctx>) -> Self {
        Self::Event(event)
    }
}

/// Receiver-side filter actor.
///
/// Registered as the `Subscriber<NetworkEvent<Ctx>>` on the real network in
/// place of the original (consensus) subscriber. Each `NetworkEvent::Proposal`
/// is checked against `drop_inbound_proposals`; when the trigger fires, the
/// event is dropped silently. Every other event is forwarded unchanged to
/// the downstream subscriber.
///
/// The actor itself carries only the (cheaply cloneable) trigger; the mutable
/// `StdRng` and the non-`Sync` downstream subscriber live in [`InboundFilterState`]
/// so the actor type satisfies `Sync` as required by `ractor::Actor`.
pub struct InboundFilter<Ctx: Context> {
    drop_inbound_proposals: Trigger,
    _ctx: std::marker::PhantomData<fn() -> Ctx>,
}

pub struct InboundFilterState<Ctx: Context> {
    rng: StdRng,
    downstream: Box<dyn Subscriber<NetworkEvent<Ctx>>>,
}

pub struct InboundFilterArgs<Ctx: Context> {
    pub seed: Option<u64>,
    pub downstream: Box<dyn Subscriber<NetworkEvent<Ctx>>>,
}

#[async_trait]
impl<Ctx: Context> Actor for InboundFilter<Ctx> {
    type Msg = InboundFilterMsg<Ctx>;
    type State = InboundFilterState<Ctx>;
    type Arguments = InboundFilterArgs<Ctx>;

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        Ok(InboundFilterState {
            rng: make_rng(args.seed),
            downstream: args.downstream,
        })
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        let InboundFilterMsg::Event(event) = msg;

        if let NetworkEvent::Proposal(_, ref signed_proposal) = event {
            let p = &signed_proposal.message;
            if self
                .drop_inbound_proposals
                .fires(p.height(), p.round(), &mut state.rng)
            {
                warn!(
                    height = %p.height(),
                    round = %p.round(),
                    "BYZANTINE: Dropping inbound proposal"
                );
                return Ok(());
            }
        }

        state.downstream.send(event);
        Ok(())
    }
}

impl<Ctx: Context> InboundFilter<Ctx> {
    /// Spawn the forwarder and return its `ActorRef`.
    ///
    /// The returned ref can be wrapped in `Box<dyn Subscriber<NetworkEvent<Ctx>>>`
    /// and handed to the real network via `NetworkMsg::Subscribe`.
    pub async fn spawn(
        drop_inbound_proposals: Trigger,
        downstream: Box<dyn Subscriber<NetworkEvent<Ctx>>>,
        seed: Option<u64>,
    ) -> Result<ActorRef<InboundFilterMsg<Ctx>>, ractor::SpawnErr> {
        let actor = Self {
            drop_inbound_proposals,
            _ctx: std::marker::PhantomData,
        };
        let (actor_ref, _) =
            Actor::spawn(None, actor, InboundFilterArgs { seed, downstream }).await?;
        Ok(actor_ref)
    }
}
