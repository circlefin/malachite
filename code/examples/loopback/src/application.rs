use std::sync::mpsc::Sender;
use tracing::{debug, info, span, trace, Level};

use malachite_consensus::{
    ConsensusMsg, Effect, Error, Input, Params, ProposedValue, Resume, SignedConsensusMsg, State,
    ValueToPropose,
};
use malachite_core_types::{
    CommitCertificate, Context, Height, Round, SignedMessage, Timeout, TimeoutKind, Validator,
    Validity, ValueOrigin,
};
use malachite_metrics::Metrics;

use crate::context::address::BasePeerAddress;
use crate::context::height::BaseHeight;
use crate::context::peer_set::BasePeerSet;
use crate::context::value::BaseValue;
use crate::context::BaseContext;
use crate::decision::Decision;

/// Represents an [`Input`] message that the application logic
/// at a certain peers sends to another peer, potentially to self.
pub struct Envelope {
    pub source: BasePeerAddress,
    pub destination: BasePeerAddress,
    pub payload: Input<BaseContext>,
}

/// An application is the deterministic state machine executing
/// at a specific peer.
///
/// It contains (1) a reference to a [`Sender`], which it uses
/// to transmit [`Input`]s to itself and other application instances
/// running at other peers; and (2) a reference to a [`Sender`] to
/// communicate to the outside environment (the system) each
/// [`Decision`] this local application took.
///
/// The application is a wrapper over the malachite consensus state
/// machine. It calls `malachite::process!` and handles
/// [`Effect`]s produced by the consensus library.
pub struct Application {
    pub peer_id: BasePeerAddress,

    /// Send [`Input`]s to the application running at self and other peers.
    pub network_tx: Sender<Envelope>,

    // Send [`Decision`]s to the environment, i.e., the [`System`].
    pub decision_tx: Sender<Decision>,
}

impl Application {
    pub fn init(&self, initial_validator_set: BasePeerSet) {
        let input = Input::StartHeight(BaseHeight(0), initial_validator_set);

        let envelope = Envelope {
            // Send this envelope to self
            destination: self.peer_id,
            source: self.peer_id,
            payload: input,
        };

        // This envelope will later be used in apply_input.
        self.network_tx.send(envelope).unwrap();
    }

    // Wrapper over `process!` macro to work around the confusion
    // in return types due to the loop { } inside that macro.
    pub fn apply_input(
        &self,
        input: Input<BaseContext>,
        peer_params: &Params<BaseContext>,
        metrics: &Metrics,
        peer_state: &mut State<BaseContext>,
        ctx: &BaseContext,
    ) -> Result<(), Error<BaseContext>> {
        malachite_consensus::process!(
            input: input,
            state: peer_state,
            metrics: metrics,
            with: effect =>
                self.handle_effect(peer_params, effect, ctx)
        )
    }

    fn handle_schedule_timeout(&self, t: Timeout) -> Result<Resume<BaseContext>, String> {
        let Timeout { round: _, kind } = t;

        // Special case to handle.
        // If it's a timeout for kind Commit, then handle this timeout instantly.
        // Signal to self that the timeout has elapsed.
        // This will prompt consensus to provide the effect `Decide` afterward.
        if kind == TimeoutKind::Commit {
            debug!("triggering TimeoutElapsed for Commit");

            self.network_tx
                .send(Envelope {
                    source: self.peer_id,
                    destination: self.peer_id,
                    payload: Input::TimeoutElapsed(t),
                })
                .unwrap();
        }

        Ok(Resume::Continue)
    }

    fn handle_broadcast(
        &self,
        v: SignedConsensusMsg<BaseContext>,
        peer_params: &Params<BaseContext>,
    ) -> Result<Resume<BaseContext>, String> {
        // Push the signed consensus message into the inbox of all peers
        // That's all that broadcast entails
        for destination in peer_params.initial_validator_set.peers.iter() {
            let destination_addr = destination.address();
            match v {
                SignedConsensusMsg::Vote(ref sv) => {
                    // Note: No need to broadcast the vote to self
                    if destination_addr != &self.peer_id {
                        self.network_tx
                            .send(Envelope {
                                source: self.peer_id,
                                destination: *destination_addr,
                                payload: Input::Vote(sv.clone()),
                            })
                            .unwrap()
                    }
                }
                SignedConsensusMsg::Proposal(ref sp) => {
                    self.network_tx
                        .send(Envelope {
                            source: self.peer_id,
                            destination: *destination_addr,
                            payload: Input::Proposal(sp.clone()),
                        })
                        .unwrap();

                    // Todo: This was not intuitive to find - source of confusion
                    self.network_tx
                        .send(Envelope {
                            source: self.peer_id,
                            destination: *destination_addr,
                            payload: Input::ProposedValue(
                                ProposedValue {
                                    height: sp.height,
                                    round: sp.round,
                                    valid_round: Round::Nil,
                                    validator_address: sp.proposer.clone(),
                                    value: sp.value,
                                    validity: Validity::Valid,
                                    extension: None,
                                },
                                ValueOrigin::Consensus,
                            ),
                        })
                        .unwrap();
                }
            }
        }
        Ok(Resume::Continue)
    }

    fn handle_decide(
        &self,
        certificate: CommitCertificate<BaseContext>,
        peer_params: &Params<BaseContext>,
    ) -> Result<Resume<BaseContext>, String> {
        // Let the top-level system/environment know about this decision
        self.decision_tx
            .send(Decision {
                peer: self.peer_id,
                value_id: certificate.value_id,
                height: certificate.height,
            })
            .expect("unable to send a decision");

        // Proceed to the next height
        let val_set = peer_params.initial_validator_set.clone();

        // Register the input in the inbox of this peer
        self.network_tx
            .send(Envelope {
                source: self.peer_id,
                destination: self.peer_id,
                payload: Input::StartHeight(certificate.height.increment(), val_set),
            })
            .unwrap();

        Ok(Resume::Continue)
    }

    // Control passes from consensus to the application here.
    // The app creates a value and provides it as input to Malachite
    // in the form of a `ValueToPropose` variant.
    // Register this input in the inbox of the current validator.
    fn handle_get_value(&self, h: BaseHeight, r: Round) -> Result<Resume<BaseContext>, String> {
        let value = 786 + h.0;
        let input_value = ValueToPropose {
            height: h,
            round: r,
            valid_round: Round::Nil,
            value: BaseValue(value),
            extension: None,
        };
        self.network_tx
            .send(Envelope {
                source: self.peer_id,
                destination: self.peer_id,
                payload: Input::Propose(input_value),
            })
            .unwrap();

        Ok(Resume::Continue)
    }

    fn handle_effect(
        &self,
        peer_params: &Params<BaseContext>,
        effect: Effect<BaseContext>,
        context: &BaseContext,
    ) -> Result<Resume<BaseContext>, String> {
        assert_eq!(peer_params.address, self.peer_id);

        let peer_id = peer_params.address;
        let span = span!(Level::INFO, "handle_effect", "{}", peer_id.0);
        let _enter = span.enter();

        match effect {
            Effect::ResetTimeouts => {
                trace!("ResetTimeouts");

                Ok(Resume::Continue)
            }
            Effect::CancelAllTimeouts => {
                trace!("CancelAllTimeouts");

                Ok(Resume::Continue)
            }
            Effect::CancelTimeout(_) => {
                trace!("CancelTimeout");

                Ok(Resume::Continue)
            }
            Effect::ScheduleTimeout(t) => {
                trace!("ScheduleTimeout {}", t);

                self.handle_schedule_timeout(t)
            }
            Effect::StartRound(_, _, _) => {
                trace!("StartRound");

                // Nothing in particular to keep track of

                Ok(Resume::Continue)
            }
            Effect::Broadcast(v) => {
                info!("Broadcast {}", pretty_broadcast(&v));

                self.handle_broadcast(v, peer_params)
            }
            Effect::GetValue(h, r, _) => {
                trace!("GetValue");

                self.handle_get_value(h, r)
            }
            Effect::GetValidatorSet(h) => {
                info!("GetValidatorSet({}); providing the default", h);

                // Assumption: validator sets do not change.
                let val_set = peer_params.initial_validator_set.clone();

                Ok(Resume::ValidatorSet(h, Some(val_set)))
            }
            Effect::VerifySignature(m, _) => {
                trace!("VerifySignature {}", pretty_verify_signature(m));

                // Consider implementing this to be able to capture more realistic
                // conditions.
                // Not required right now, given the current use of this application.

                Ok(Resume::SignatureValidity(true))
            }
            Effect::Decide { certificate } => {
                trace!("Decide");

                self.handle_decide(certificate, peer_params)
            }
            Effect::RestreamValue(_, _, _, _, _) => {
                panic!("unimplemented arm Effect::RestreamValue in match effect")
            }
            Effect::PersistMessage(_) => {
                // No support for crash-recovery
                Ok(Resume::Continue)
            }
            Effect::PersistTimeout(_) => {
                // No support for crash-recovery
                Ok(Resume::Continue)
            }
            Effect::SignVote(v) => {
                let sv = context.sign_vote(v);
                Ok(Resume::SignedVote(sv))
            }
            Effect::SignProposal(p) => {
                let sp = context.sign_proposal(p);
                Ok(Resume::SignedProposal(sp))
            }
            Effect::GetVoteSet(_, _) => {
                panic!("unimplemented arm Effect::GetVoteSet in match effect")
            }
            Effect::SendVoteSetResponse(_, _, _, _) => {
                panic!("unimplemented arm Effect::SendVoteSetResponse in match effect")
            }
        }
    }
}

fn pretty_broadcast(v: &SignedConsensusMsg<BaseContext>) -> String {
    match v {
        SignedConsensusMsg::Vote(ref sv) => sv.to_string(),
        SignedConsensusMsg::Proposal(ref sp) => sp.to_string(),
    }
}

fn pretty_verify_signature(m: SignedMessage<BaseContext, ConsensusMsg<BaseContext>>) -> String {
    match m.message {
        ConsensusMsg::Vote(v) => v.to_string(),
        ConsensusMsg::Proposal(p) => p.to_string(),
    }
}
