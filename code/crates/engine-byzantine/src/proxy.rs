//! Byzantine network proxy actor.
//!
//! [`ByzantineNetworkProxy`] is a ractor actor that sits between the engine's
//! `Consensus` actor and the real network actor. It intercepts outgoing
//! [`NetworkMsg::PublishConsensusMsg`] and [`NetworkMsg::PublishLivenessMsg`]
//! messages and can:
//!
//! - **Drop** vote/proposal messages (simulating silence / censorship)
//! - **Duplicate** vote/proposal messages with conflicting content on consensus
//!   and liveness vote paths (simulating equivocation)
//! - **Forward** non-targeted messages unchanged (honest behavior)
//!
//! Subscribe messages receive special handling: when `drop_inbound_proposals`
//! is configured, an [`InboundFilter`] is spliced between the real network's
//! output port and the consensus subscriber so selected inbound proposals can
//! be dropped on the receive path. Otherwise the subscribe is forwarded
//! transparently.
//!
//! All other message types are forwarded transparently to the real network.

use std::collections::HashMap;

use async_trait::async_trait;
use eyre::{eyre, Result};
use ractor::{Actor, ActorProcessingErr, ActorRef};
use rand::rngs::StdRng;
use tracing::{debug, error, warn, Instrument};

use malachitebft_core_consensus::{LivenessMsg, SignedConsensusMsg};
use malachitebft_core_types::{
    Context, Height, NilOrVal, Proposal, Round, ValueId, Vote, VoteType,
};
use malachitebft_engine::network::{Msg as NetworkMsg, NetworkRef};
use malachitebft_signing::Signer;

use crate::config::{make_rng, ByzantineConfig};
use crate::inbound::InboundFilter;

/// A function that creates a conflicting value from an original one.
///
/// Used for proposal equivocation: the proxy sends the original proposal
/// and then a second proposal with the value returned by this function.
pub type ConflictingValueFn<Ctx> =
    Box<dyn Fn(&<Ctx as Context>::Value) -> <Ctx as Context>::Value + Send + Sync>;

/// A function that creates a conflicting vote value ID.
///
/// Receives the original value ID (`Some` for non-nil votes, `None` for nil
/// votes) and returns a value ID for the conflicting vote.
pub type ConflictingVoteValueFn<Ctx> =
    Box<dyn Fn(Option<&ValueId<Ctx>>) -> ValueId<Ctx> + Send + Sync>;

/// A ractor actor that proxies [`NetworkMsg`] between consensus and the real
/// network, applying Byzantine behavior according to a [`ByzantineConfig`].
///
/// Because it handles the same `Msg<Ctx>` message type as the `Network` actor,
/// its `ActorRef` is a `NetworkRef<Ctx>` and can be used as a drop-in
/// replacement when constructing the consensus actor.
pub struct ByzantineNetworkProxy<Ctx: Context> {
    config: ByzantineConfig,
    real_network: NetworkRef<Ctx>,
    signer: Box<dyn Signer<Ctx>>,
    ctx: Ctx,
    address: Ctx::Address,
    span: tracing::Span,
    /// Factory for creating a conflicting value for proposal equivocation.
    /// Required for proposal equivocation to take effect; if `None`, equivocation is skipped.
    conflicting_value_fn: Option<ConflictingValueFn<Ctx>>,
    /// Factory for creating a conflicting value ID for vote equivocation.
    /// Receives `Some(&id)` for non-nil votes, `None` for nil votes.
    /// If absent, vote equivocation falls back to flipping `Val -> Nil`.
    conflicting_vote_value_fn: Option<ConflictingVoteValueFn<Ctx>>,
}

/// The action decided for a vote by the consensus path, to be replayed
/// on the liveness path for the same vote.
#[derive(Clone, Copy, Debug)]
enum VoteAction {
    Drop,
    Equivocate,
    Forward,
}

/// Internal mutable state for the proxy actor.
pub struct ProxyState {
    rng: StdRng,
    /// Caches the action decided for each vote on the consensus path, keyed
    /// by `(height, round, vote_type)`. The liveness path replays the cached
    /// decision instead of re-evaluating the trigger (which would consume a
    /// fresh RNG sample and produce an independent, inconsistent result).
    vote_actions: HashMap<(u64, i64, VoteType), VoteAction>,
}

impl<Ctx: Context> ByzantineNetworkProxy<Ctx> {
    /// Spawn the proxy actor and return its ref (which is a `NetworkRef<Ctx>`).
    ///
    /// Optional factories customize equivocation behavior:
    /// - Without `conflicting_value_fn`, proposal equivocation is skipped.
    /// - Without `conflicting_vote_value_fn`, non-nil votes equivocate to nil;
    ///   nil votes cannot be equivocated.
    #[allow(clippy::too_many_arguments)]
    pub async fn spawn(
        config: ByzantineConfig,
        real_network: NetworkRef<Ctx>,
        signer: Box<dyn Signer<Ctx>>,
        ctx: Ctx,
        address: Ctx::Address,
        span: tracing::Span,
        conflicting_value_fn: Option<ConflictingValueFn<Ctx>>,
        conflicting_vote_value_fn: Option<ConflictingVoteValueFn<Ctx>>,
    ) -> Result<NetworkRef<Ctx>> {
        config
            .validate()
            .map_err(|e| eyre!("Invalid ByzantineConfig: {e}"))?;

        let seed = config.seed;
        let proxy = Self {
            config,
            real_network,
            signer,
            ctx,
            address,
            span,
            conflicting_value_fn,
            conflicting_vote_value_fn,
        };

        let (actor_ref, _) = Actor::spawn(None, proxy, seed)
            .await
            .map_err(|e| eyre!("Failed to spawn ByzantineNetworkProxy: {e}"))?;

        Ok(actor_ref)
    }
}

#[async_trait]
impl<Ctx: Context> Actor for ByzantineNetworkProxy<Ctx> {
    type Msg = NetworkMsg<Ctx>;
    type State = ProxyState;
    type Arguments = Option<u64>; // seed

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        seed: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        Ok(ProxyState {
            rng: make_rng(seed),
            vote_actions: HashMap::new(),
        })
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            NetworkMsg::PublishConsensusMsg(ref consensus_msg) => {
                self.handle_consensus_msg(consensus_msg, state)
                    .instrument(self.span.clone())
                    .await?;
            }
            NetworkMsg::PublishLivenessMsg(ref liveness_msg) => {
                self.handle_liveness_msg(liveness_msg, state)
                    .instrument(self.span.clone())
                    .await?;
            }
            // Receiver-side interception. When `drop_inbound_proposals` is set,
            // splice an `InboundFilter` forwarder between the real network's
            // output port and the consensus subscriber so that specific
            // `NetworkEvent::Proposal` events can be dropped on the way in.
            // When the trigger is unset we just forward the original subscriber
            // unchanged (zero overhead in the honest path).
            NetworkMsg::Subscribe(inner) => {
                let _enter = self.span.enter();
                if self.config.drop_inbound_proposals.is_set() {
                    warn!(
                        trigger = ?self.config.drop_inbound_proposals,
                        "BYZANTINE: Installing inbound-proposal filter"
                    );
                    let filter_ref = InboundFilter::<Ctx>::spawn(
                        self.config.drop_inbound_proposals.clone(),
                        inner,
                        self.config.seed,
                    )
                    .await
                    .map_err(|e| format!("Failed to spawn InboundFilter: {e:?}"))?;
                    self.real_network
                        .cast(NetworkMsg::Subscribe(Box::new(filter_ref)))
                        .map_err(|e| format!("Failed to forward Subscribe: {e:?}"))?;
                } else {
                    self.real_network
                        .cast(NetworkMsg::Subscribe(inner))
                        .map_err(|e| format!("Failed to forward Subscribe: {e:?}"))?;
                }
            }
            // All other network messages are forwarded unchanged. Some of them
            // carry `RpcReplyPort`s inside the message payload (for example
            // `OutgoingRequest`, `DumpState`, and `UpdatePersistentPeers`), and
            // `cast` still works because the reply port travels with the
            // forwarded message itself. The proxy only intercepts outbound
            // requests, so it does not need to handle replies separately.
            other => {
                let _enter = self.span.enter();
                self.real_network
                    .cast(other)
                    .map_err(|e| format!("Failed to forward message to network: {e:?}"))?;
            }
        }

        Ok(())
    }
}

impl<Ctx: Context> ByzantineNetworkProxy<Ctx> {
    async fn handle_consensus_msg(
        &self,
        msg: &SignedConsensusMsg<Ctx>,
        state: &mut ProxyState,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            SignedConsensusMsg::Vote(signed_vote) => {
                let vote = &signed_vote.message;
                let height = vote.height();
                let round = vote.round();
                let vote_key = (height.as_u64(), round.as_i64(), vote.vote_type());

                // Check drop trigger first
                if self.config.drop_votes.fires(height, round, &mut state.rng) {
                    warn!(%height, %round, vote_type = ?vote.vote_type(), "BYZANTINE: Dropping vote");
                    state.vote_actions.insert(vote_key, VoteAction::Drop);
                    return Ok(());
                }

                // Check equivocation trigger
                if self
                    .config
                    .equivocate_votes
                    .fires(height, round, &mut state.rng)
                {
                    warn!(%height, %round, vote_type = ?vote.vote_type(), "BYZANTINE: Equivocating vote");
                    state.vote_actions.insert(vote_key, VoteAction::Equivocate);

                    // Send the original vote
                    self.forward_consensus_msg(msg)?;

                    // Construct and send a conflicting vote
                    self.sign_and_send_conflicting_vote(vote, |signed| {
                        NetworkMsg::PublishConsensusMsg(SignedConsensusMsg::Vote(signed))
                    })
                    .await?;

                    return Ok(());
                }

                // Default: forward as-is
                state.vote_actions.insert(vote_key, VoteAction::Forward);
                debug!(%height, %round, "Forwarding vote");
                self.forward_consensus_msg(msg)?;
            }

            SignedConsensusMsg::Proposal(signed_proposal) => {
                let proposal = &signed_proposal.message;
                let height = proposal.height();
                let round = proposal.round();

                // Check drop trigger first
                if self
                    .config
                    .drop_proposals
                    .fires(height, round, &mut state.rng)
                {
                    warn!(%height, %round, "BYZANTINE: Dropping proposal");
                    return Ok(());
                }

                // Check equivocation trigger
                if self
                    .config
                    .equivocate_proposals
                    .fires(height, round, &mut state.rng)
                {
                    // Send the original proposal first
                    self.forward_consensus_msg(msg)?;

                    // Construct and send a conflicting proposal
                    // TODO: In ProposalAndParts mode, proposal equivocation only
                    // duplicates the SignedProposal. To make peers process both
                    // values as full proposals and emit proposal evidence, this
                    // path also needs to send a matching conflicting proposal
                    // part stream (or otherwise inject the corresponding
                    // ProposedValue) for the conflicting value.
                    self.send_conflicting_proposal(proposal).await.map_err(|e| {
                        error!(%e, "Failed to send conflicting proposal after original was sent");
                        ActorProcessingErr::from(e.to_string())
                    })?;

                    return Ok(());
                }

                // Default: forward as-is
                debug!(%height, %round, "Forwarding proposal");
                self.forward_consensus_msg(msg)?;
            }
        }

        Ok(())
    }

    /// Handle a liveness message, applying drop and equivocation rules for votes.
    ///
    /// Liveness messages carry rebroadcast votes and certificates. Without
    /// filtering these, a Byzantine node configured to drop or equivocate
    /// votes would still have its votes delivered unmodified to peers
    /// through the liveness channel.
    async fn handle_liveness_msg(
        &self,
        msg: &LivenessMsg<Ctx>,
        state: &mut ProxyState,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            LivenessMsg::Vote(signed_vote) => {
                let vote = &signed_vote.message;
                let height = vote.height();
                let round = vote.round();
                let vote_key = (height.as_u64(), round.as_i64(), vote.vote_type());

                // Replay the action decided on the consensus path for this vote.
                // If no cached action exists (e.g., the vote was only seen on
                // liveness), evaluate the triggers fresh.
                let action = state
                    .vote_actions
                    .get(&vote_key)
                    .copied()
                    .unwrap_or_else(|| self.decide_vote_action(height, round, &mut state.rng));

                match action {
                    VoteAction::Drop => {
                        warn!(%height, %round, vote_type = ?vote.vote_type(), "BYZANTINE: Dropping liveness vote");
                    }
                    VoteAction::Equivocate => {
                        warn!(%height, %round, vote_type = ?vote.vote_type(), "BYZANTINE: Equivocating liveness vote");

                        // Send the original vote first.
                        self.forward_liveness_msg(msg)?;

                        // Construct and send a conflicting liveness vote.
                        self.sign_and_send_conflicting_vote(vote, |signed| {
                            NetworkMsg::PublishLivenessMsg(LivenessMsg::Vote(signed))
                        })
                        .await?;
                    }
                    VoteAction::Forward => {
                        self.forward_liveness_msg(msg)?;
                    }
                }
            }
            // TODO: Add equivocation logic for PolkaCertificate and SkipRoundCertificate.
            // Possible attacks:
            //   - Drop certificates
            //   - Cross-round signature replay on RoundCertificate: reuse signatures
            //     from a previously cached certificate.
            _ => {
                self.forward_liveness_msg(msg)?;
            }
        }

        Ok(())
    }

    /// Evaluate the drop and equivocation triggers for a vote, returning
    /// the decided action. Used as a fallback when no cached action exists.
    fn decide_vote_action(
        &self,
        height: impl malachitebft_core_types::Height,
        round: Round,
        rng: &mut StdRng,
    ) -> VoteAction {
        if self.config.drop_votes.fires(height, round, rng) {
            VoteAction::Drop
        } else if self.config.equivocate_votes.fires(height, round, rng) {
            VoteAction::Equivocate
        } else {
            VoteAction::Forward
        }
    }

    /// Forward a consensus message to the real network.
    fn forward_consensus_msg(
        &self,
        msg: &SignedConsensusMsg<Ctx>,
    ) -> Result<(), ActorProcessingErr> {
        self.real_network
            .cast(NetworkMsg::PublishConsensusMsg(msg.clone()))
            .map_err(|e| {
                ActorProcessingErr::from(format!(
                    "Failed to forward consensus message to network: {e:?}"
                ))
            })
    }

    /// Forward a liveness message to the real network.
    fn forward_liveness_msg(&self, msg: &LivenessMsg<Ctx>) -> Result<(), ActorProcessingErr> {
        self.real_network
            .cast(NetworkMsg::PublishLivenessMsg(msg.clone()))
            .map_err(|e| {
                ActorProcessingErr::from(format!(
                    "Failed to forward liveness message to network: {e:?}"
                ))
            })
    }

    /// Construct a conflicting proposal and send it.
    ///
    /// Requires a [`ConflictingValueFn`] to create a proposal with a different
    /// value. If no factory was provided, equivocation is skipped.
    async fn send_conflicting_proposal(&self, original: &Ctx::Proposal) -> Result<()> {
        let height = original.height();
        let round = original.round();
        let pol_round = original.pol_round();

        let Some(ref make_value) = self.conflicting_value_fn else {
            warn!(%height, %round, "BYZANTINE: Skipping proposal equivocation (no ConflictingValueFn provided)");
            return Ok(());
        };

        let conflicting_value = make_value(original.value());
        warn!(%height, %round, "BYZANTINE: Sending conflicting proposal with different value");
        let conflicting_proposal = self.ctx.new_proposal(
            height,
            round,
            conflicting_value,
            pol_round,
            self.address.clone(),
        );

        let signed = self
            .signer
            .sign_proposal(conflicting_proposal)
            .await
            .map_err(|e| eyre!("Failed to sign conflicting proposal: {e}"))?;

        self.real_network
            .cast(NetworkMsg::PublishConsensusMsg(
                SignedConsensusMsg::Proposal(signed),
            ))
            .map_err(|e| eyre!("Failed to send conflicting proposal to network: {e:?}"))?;

        Ok(())
    }

    /// Construct a conflicting vote.
    ///
    /// If a [`ConflictingVoteValueFn`] was provided, creates a vote with the
    /// value ID it returns. The factory receives `Some(&id)` for non-nil votes
    /// and `None` for nil votes. Without a factory, flips `Val -> Nil`
    /// (nil votes without a factory cannot be equivocated).
    fn make_conflicting_vote(&self, original: &Ctx::Vote) -> Option<Ctx::Vote> {
        let height = original.height();
        let round = original.round();
        let vote_type = original.vote_type();

        let conflicting_value = match (original.value(), &self.conflicting_vote_value_fn) {
            (NilOrVal::Val(value_id), Some(make_value_id)) => {
                let conflicting_value_id = make_value_id(Some(value_id));
                warn!(%height, %round, "BYZANTINE: Sending conflicting vote with different value");
                NilOrVal::Val(conflicting_value_id)
            }
            (NilOrVal::Val(_), None) => {
                warn!(%height, %round, "BYZANTINE: No conflicting vote value factory configured, falling back to nil vote");
                NilOrVal::Nil
            }
            (NilOrVal::Nil, Some(make_value_id)) => {
                let conflicting_value_id = make_value_id(None);
                warn!(%height, %round, "BYZANTINE: Equivocating nil vote with fabricated value");
                NilOrVal::Val(conflicting_value_id)
            }
            (NilOrVal::Nil, None) => {
                warn!(%height, %round, "BYZANTINE: Cannot equivocate a nil vote (no value to flip to), skipping");
                return None;
            }
        };

        let vote = match vote_type {
            VoteType::Prevote => {
                self.ctx
                    .new_prevote(height, round, conflicting_value, self.address.clone())
            }
            VoteType::Precommit => {
                self.ctx
                    .new_precommit(height, round, conflicting_value, self.address.clone())
            }
        };

        Some(vote)
    }

    /// Sign a conflicting vote and send it wrapped by the given `wrap` function.
    async fn sign_and_send_conflicting_vote(
        &self,
        original: &Ctx::Vote,
        wrap: impl FnOnce(malachitebft_core_types::SignedVote<Ctx>) -> NetworkMsg<Ctx>,
    ) -> Result<()> {
        let Some(conflicting_vote) = self.make_conflicting_vote(original) else {
            warn!(height = %original.height(), round = %original.round(), vote_type = ?original.vote_type(), "BYZANTINE: Skipping nil vote equivocation");
            return Ok(());
        };

        let signed = self
            .signer
            .sign_vote(conflicting_vote)
            .await
            .map_err(|e| eyre!("Failed to sign conflicting vote: {e}"))?;

        self.real_network
            .cast(wrap(signed))
            .map_err(|e| eyre!("Failed to send conflicting vote to network: {e:?}"))
    }
}
