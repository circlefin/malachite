//! Byzantine network proxy actor.
//!
//! [`ByzantineNetworkProxy`] is a ractor actor that sits between the consensus
//! actor and the real network actor. It intercepts outgoing
//! [`NetworkMsg::PublishConsensusMsg`] and [`NetworkMsg::PublishLivenessMsg`]
//! messages and can:
//!
//! - **Drop** messages (simulating silence / censorship)
//! - **Duplicate** messages with conflicting content (simulating equivocation)
//! - **Forward** messages unchanged (honest behavior)
//!
//! All other message types are forwarded transparently to the real network.

use async_trait::async_trait;
use eyre::eyre;
use ractor::{Actor, ActorProcessingErr, ActorRef};
use rand::rngs::StdRng;
use tracing::{debug, warn};

use malachitebft_core_consensus::{LivenessMsg, SignedConsensusMsg};
use malachitebft_core_types::{Context, NilOrVal, Proposal, Round, Vote, VoteType};
use malachitebft_engine::network::{Msg as NetworkMsg, NetworkRef};
use malachitebft_signing::SigningProvider;

use crate::config::{make_rng, ByzantineConfig};

/// A function that creates a conflicting value from an original one.
///
/// Used for proposal equivocation: the proxy sends the original proposal
/// and then a second proposal with the value returned by this function.
pub type ConflictingValueFn<Ctx> =
    Box<dyn Fn(&<Ctx as Context>::Value) -> <Ctx as Context>::Value + Send + Sync>;

/// A ractor actor that proxies [`NetworkMsg`] between consensus and the real
/// network, applying Byzantine behavior according to a [`ByzantineConfig`].
///
/// Because it handles the same `Msg<Ctx>` message type as the `Network` actor,
/// its `ActorRef` is a `NetworkRef<Ctx>` and can be used as a drop-in
/// replacement when constructing the consensus actor.
pub struct ByzantineNetworkProxy<Ctx: Context> {
    config: ByzantineConfig,
    real_network: NetworkRef<Ctx>,
    signing_provider: Box<dyn SigningProvider<Ctx>>,
    ctx: Ctx,
    address: Ctx::Address,
    span: tracing::Span,
    /// Optional factory for creating a conflicting value for proposal equivocation.
    /// If `None`, proposal equivocation sends the original proposal with a flipped `pol_round`.
    conflicting_value_fn: Option<ConflictingValueFn<Ctx>>,
}

/// Internal mutable state for the proxy actor.
pub struct ProxyState {
    rng: StdRng,
}

impl<Ctx: Context> ByzantineNetworkProxy<Ctx> {
    /// Spawn the proxy actor and return its ref (which is a `NetworkRef<Ctx>`).
    ///
    /// The optional `conflicting_value_fn` is used for proposal equivocation:
    /// given the original proposed value, it returns a different value to use
    /// in the conflicting proposal. If `None`, proposal equivocation sends a
    /// proposal with a flipped `pol_round` instead.
    pub async fn spawn(
        config: ByzantineConfig,
        real_network: NetworkRef<Ctx>,
        signing_provider: Box<dyn SigningProvider<Ctx>>,
        ctx: Ctx,
        address: Ctx::Address,
        span: tracing::Span,
        conflicting_value_fn: Option<ConflictingValueFn<Ctx>>,
    ) -> Result<NetworkRef<Ctx>, eyre::Report> {
        let seed = config.seed;
        let proxy = Self {
            config,
            real_network,
            signing_provider,
            ctx,
            address,
            span,
            conflicting_value_fn,
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
        })
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        let _enter = self.span.enter();

        match msg {
            NetworkMsg::PublishConsensusMsg(ref consensus_msg) => {
                self.handle_consensus_msg(consensus_msg, state).await?;
            }
            NetworkMsg::PublishLivenessMsg(ref liveness_msg) => {
                self.handle_liveness_msg(liveness_msg, state)?;
            }
            // All other message types are forwarded transparently.
            other => {
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

                // Check drop trigger first
                if let Some(ref trigger) = self.config.drop_votes {
                    if trigger.fires(height, round, &mut state.rng) {
                        warn!(
                            %height, %round,
                            vote_type = ?vote.vote_type(),
                            "BYZANTINE: Dropping vote"
                        );
                        return Ok(());
                    }
                }

                // Check equivocation trigger
                if let Some(ref trigger) = self.config.equivocate_votes {
                    if trigger.fires(height, round, &mut state.rng) {
                        warn!(
                            %height, %round,
                            vote_type = ?vote.vote_type(),
                            "BYZANTINE: Equivocating vote"
                        );

                        // Send the original vote
                        self.forward_consensus_msg(msg)?;

                        // Construct and send a conflicting vote
                        if let Err(e) = self.send_conflicting_vote(vote).await {
                            warn!("Failed to send conflicting vote: {e}");
                        }

                        return Ok(());
                    }
                }

                // Default: forward as-is
                debug!(%height, %round, "Forwarding vote");
                self.forward_consensus_msg(msg)?;
            }

            SignedConsensusMsg::Proposal(signed_proposal) => {
                let proposal = &signed_proposal.message;
                let height = proposal.height();
                let round = proposal.round();

                // Check drop trigger first
                if let Some(ref trigger) = self.config.drop_proposals {
                    if trigger.fires(height, round, &mut state.rng) {
                        warn!(
                            %height, %round,
                            "BYZANTINE: Dropping proposal"
                        );
                        return Ok(());
                    }
                }

                // Check equivocation trigger
                if let Some(ref trigger) = self.config.equivocate_proposals {
                    if trigger.fires(height, round, &mut state.rng) {
                        warn!(
                            %height, %round,
                            "BYZANTINE: Equivocating proposal"
                        );

                        // Send the original proposal first
                        self.forward_consensus_msg(msg)?;

                        // Construct and send a conflicting proposal
                        if let Err(e) = self.send_conflicting_proposal(proposal).await {
                            warn!("Failed to send conflicting proposal: {e}");
                        }

                        return Ok(());
                    }
                }

                // Default: forward as-is
                debug!(%height, %round, "Forwarding proposal");
                self.forward_consensus_msg(msg)?;
            }
        }

        Ok(())
    }

    /// Handle a liveness message, applying drop rules for votes.
    ///
    /// Liveness messages carry rebroadcast votes and certificates. Without
    /// filtering these, a Byzantine node configured to drop votes would still
    /// have its votes delivered to peers through the liveness channel.
    fn handle_liveness_msg(
        &self,
        msg: &LivenessMsg<Ctx>,
        state: &mut ProxyState,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            LivenessMsg::Vote(signed_vote) => {
                let vote = &signed_vote.message;
                let height = vote.height();
                let round = vote.round();

                if let Some(ref trigger) = self.config.drop_votes {
                    if trigger.fires(height, round, &mut state.rng) {
                        warn!(
                            %height, %round,
                            vote_type = ?vote.vote_type(),
                            "BYZANTINE: Dropping liveness vote"
                        );
                        return Ok(());
                    }
                }

                self.forward_liveness_msg(msg)?;
            }
            // Other liveness messages (certificates) are forwarded as-is.
            _ => {
                self.forward_liveness_msg(msg)?;
            }
        }

        Ok(())
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
    /// If a [`ConflictingValueFn`] was provided, creates a proposal with a
    /// different value. Otherwise, creates a proposal with a flipped `pol_round`
    /// (Nil if original had a pol_round, Round(0) if original was Nil).
    async fn send_conflicting_proposal(
        &self,
        original: &Ctx::Proposal,
    ) -> Result<(), eyre::Report> {
        let height = original.height();
        let round = original.round();
        let pol_round = original.pol_round();

        let conflicting_proposal = if let Some(ref make_value) = self.conflicting_value_fn {
            let conflicting_value = make_value(original.value());
            warn!(
                %height, %round,
                "BYZANTINE: Sending conflicting proposal with different value"
            );
            self.ctx.new_proposal(
                height,
                round,
                conflicting_value,
                pol_round,
                self.address.clone(),
            )
        } else {
            // No value factory: flip pol_round to create a structurally different proposal.
            let conflicting_pol_round = if pol_round == Round::Nil {
                Round::new(0)
            } else {
                Round::Nil
            };
            warn!(
                %height, %round,
                "BYZANTINE: Sending conflicting proposal with flipped pol_round \
                 (no conflicting value factory configured)"
            );
            self.ctx.new_proposal(
                height,
                round,
                original.value().clone(),
                conflicting_pol_round,
                self.address.clone(),
            )
        };

        let signed = self
            .signing_provider
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

    /// Construct a conflicting vote (flipping value <-> nil) and send it.
    async fn send_conflicting_vote(&self, original: &Ctx::Vote) -> Result<(), eyre::Report> {
        let height = original.height();
        let round = original.round();
        let vote_type = original.vote_type();

        // Flip the value: if the original votes for a value, vote nil; if nil, we can't
        // easily construct a conflicting value vote without knowing a valid value ID,
        // so we just skip equivocation for nil votes.
        let conflicting_value = match original.value() {
            NilOrVal::Val(_) => NilOrVal::Nil,
            NilOrVal::Nil => {
                debug!(
                    %height, %round,
                    "Cannot equivocate a nil vote (no value to flip to), skipping"
                );
                return Ok(());
            }
        };

        let conflicting_vote = match vote_type {
            VoteType::Prevote => {
                self.ctx
                    .new_prevote(height, round, conflicting_value, self.address.clone())
            }
            VoteType::Precommit => {
                self.ctx
                    .new_precommit(height, round, conflicting_value, self.address.clone())
            }
        };

        let signed = self
            .signing_provider
            .sign_vote(conflicting_vote)
            .await
            .map_err(|e| eyre!("Failed to sign conflicting vote: {e}"))?;

        self.real_network
            .cast(NetworkMsg::PublishConsensusMsg(SignedConsensusMsg::Vote(
                signed,
            )))
            .map_err(|e| eyre!("Failed to send conflicting vote to network: {e:?}"))?;

        Ok(())
    }
}
