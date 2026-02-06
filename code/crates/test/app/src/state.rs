//! Internal state of the application. This is a simplified abstract to keep it simple.
//! A regular application would have mempool implemented, a proper database and input methods like RPC.

use std::collections::HashSet;
use std::fmt;
use std::sync::Arc;

use bytes::Bytes;
use eyre::eyre;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use sha3::Digest;
use tracing::{debug, error, info};

use malachitebft_app_channel::app::consensus::{ProposedValue, Role};
use malachitebft_app_channel::app::streaming::{StreamContent, StreamId, StreamMessage};
use malachitebft_app_channel::app::types::codec::Codec;
use malachitebft_app_channel::app::types::core::{CommitCertificate, Round, Validity};
use malachitebft_app_channel::app::types::{LocallyProposedValue, PeerId};
use malachitebft_test::codec::json::JsonCodec;
use malachitebft_test::middleware::Middleware;
use malachitebft_test::{
    Address, Ed25519Provider, Genesis, Height, LinearTimeouts, ProposalData, ProposalFin,
    ProposalInit, ProposalPart, TestContext, ValidatorSet, Value, ValueId,
};

use crate::config::Config;
use crate::store::{DecidedValue, Store};
use crate::streaming::{PartStreamsMap, ProposalParts};

/// Number of historical values to keep in the store
const HISTORY_LENGTH: u64 = 500;

/// Represents the internal state of the application node
/// Contains information about current height, round, proposals and blocks
pub struct State {
    pub ctx: TestContext,
    pub config: Config,
    pub genesis: Genesis,
    pub address: Address,
    pub current_height: Height,
    pub current_round: Round,
    pub current_proposer: Option<Address>,
    pub current_role: Role,
    pub peers: HashSet<PeerId>,
    pub store: Store,
    pub middleware: Option<Arc<dyn Middleware>>,

    signing_provider: Ed25519Provider,
    streams_map: PartStreamsMap,
    rng: StdRng,
}

/// Represents errors that can occur during the verification of a proposal's signature.
#[derive(Debug)]
pub enum SignatureVerificationError {
    /// Indicates that the `Init` part of the proposal is unexpectedly missing.
    MissingInitPart,
    /// Indicates that the `Fin` part of the proposal is unexpectedly missing.
    MissingFinPart,
    /// Indicates that the proposer was not found in the validator set.
    ProposerNotFound,
    /// Indicates that the signature in the `Fin` part is invalid.
    InvalidSignature,
}

/// Represents errors that can occur during proposal validation
#[derive(Debug)]
// To suppress warning about unused SignatureVerificationError, we use it via derive(Debug)
#[allow(dead_code)]
pub enum ProposalValidationError {
    /// Proposer doesn't match the expected proposer for the given round
    WrongProposer { actual: Address, expected: Address },
    /// Signature verification errors
    Signature(SignatureVerificationError),
}

impl fmt::Display for ProposalValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProposalValidationError::WrongProposer { actual, expected } => {
                write!(f, "Wrong proposer: got {}, expected {}", actual, expected)
            }
            ProposalValidationError::Signature(err) => {
                write!(f, "Signature verification failed: {:?}", err)
            }
        }
    }
}

impl State {
    /// Creates a new State instance with the given validator address and starting height
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        ctx: TestContext,
        config: Config,
        genesis: Genesis,
        address: Address,
        height: Height,
        store: Store,
        signing_provider: Ed25519Provider,
        middleware: Option<Arc<dyn Middleware>>,
    ) -> Self {
        Self {
            ctx,
            config,
            genesis,
            address,
            store,
            signing_provider,
            middleware,
            current_height: height,
            current_round: Round::new(0),
            current_proposer: None,
            current_role: Role::None,
            streams_map: PartStreamsMap::new(),
            rng: StdRng::from_entropy(),
            peers: HashSet::new(),
        }
    }

    /// Returns the set of validators for the given height.
    pub fn get_validator_set(&self, height: Height) -> ValidatorSet {
        self.ctx
            .middleware()
            .get_validator_set(&self.ctx, self.current_height, height, &self.genesis)
            .unwrap_or_else(|| self.genesis.validator_set.clone())
    }

    /// Returns the timeouts for the given height.
    pub fn get_timeouts(&self, height: Height) -> LinearTimeouts {
        self.ctx
            .middleware()
            .get_timeouts(&self.ctx, self.current_height, height)
            .unwrap_or_default()
    }

    /// Returns the earliest height available in the state
    pub async fn get_earliest_height(&self) -> Height {
        self.store
            .min_decided_value_height()
            .await
            .unwrap_or_default()
    }

    /// Validates a proposal by checking both proposer and signature
    pub fn validate_proposal_parts(
        &self,
        parts: &ProposalParts,
    ) -> Result<(), ProposalValidationError> {
        let height = parts.height;
        let round = parts.round;

        // Get the expected proposer for this height and round
        let validator_set = self.get_validator_set(height);

        let expected_proposer = self
            .ctx
            .select_proposer(&validator_set, height, round)
            .address;

        // Check if the proposer matches the expected proposer
        if parts.proposer != expected_proposer {
            return Err(ProposalValidationError::WrongProposer {
                actual: parts.proposer,
                expected: expected_proposer,
            });
        }

        // If proposer is correct, verify the signature
        self.verify_proposal_parts_signature(parts)
            .map_err(ProposalValidationError::Signature)?;

        Ok(())
    }

    /// Verify proposal signature
    fn verify_proposal_parts_signature(
        &self,
        parts: &ProposalParts,
    ) -> Result<(), SignatureVerificationError> {
        let mut hasher = sha3::Keccak256::new();

        let init = parts
            .init()
            .ok_or(SignatureVerificationError::MissingInitPart)?;

        let fin = parts
            .fin()
            .ok_or(SignatureVerificationError::MissingFinPart)?;

        let hash = {
            hasher.update(init.height.as_u64().to_be_bytes());
            hasher.update(init.round.as_i64().to_be_bytes());

            // The correctness of the hash computation relies on the parts being ordered by sequence
            // number, which is guaranteed by the `PartStreamsMap`.
            for part in parts.parts.iter().filter_map(|part| part.as_data()) {
                hasher.update(part.factor.to_be_bytes());
            }

            hasher.finalize()
        };

        // Retrieve the proposer from the validator set for the given height
        let validator_set = self.get_validator_set(parts.height);

        let proposer = validator_set
            .get_by_address(&parts.proposer)
            .ok_or(SignatureVerificationError::ProposerNotFound)?;

        // Verify the signature
        if !self
            .signing_provider
            .verify(&hash, &fin.signature, &proposer.public_key)
        {
            return Err(SignatureVerificationError::InvalidSignature);
        }

        Ok(())
    }

    /// Processes and adds a new proposal to the state if it's valid
    /// Returns Some(ProposedValue) if the proposal was accepted, None otherwise
    pub async fn received_proposal_part(
        &mut self,
        from: PeerId,
        part: StreamMessage<ProposalPart>,
    ) -> eyre::Result<Option<ProposedValue<TestContext>>> {
        let sequence = part.sequence;

        // Check if we have a full proposal
        let Some(parts) = self.streams_map.insert(from, part) else {
            return Ok(None);
        };

        // Check if the proposal is outdated
        if parts.height < self.current_height {
            debug!(
                height = %self.current_height,
                round = %self.current_round,
                part.height = %parts.height,
                part.round = %parts.round,
                part.sequence = %sequence,
                "Received outdated proposal, ignoring"
            );
            return Ok(None);
        }

        // Store future proposals parts in pending without validation
        if parts.height > self.current_height {
            info!(%parts.height, %parts.round, "Storing proposal parts for a future height in pending");
            self.store.store_pending_proposal_parts(parts).await?;
            return Ok(None);
        }

        // For current height, validate proposal (proposer + signature)
        match self.validate_proposal_parts(&parts) {
            Ok(()) => {
                // Validation passed - assemble and store as undecided
                let mut value = Self::assemble_value_from_parts(parts)?;

                // Use middleware to determine validity (allows testing custom validation logic)
                if let Some(middleware) = &self.middleware {
                    let new_validity =
                        middleware.get_validity(&self.ctx, value.height, value.round, &value.value);
                    info!(%value.height, "Middleware returned validity: {:?}", new_validity);
                    value.validity = new_validity;
                }

                info!(%value.height, %value.round, %value.proposer, validity = ?value.validity, "Storing validated proposal as undecided");
                self.store.store_undecided_proposal(value.clone()).await?;
                Ok(Some(value))
            }
            Err(error) => {
                // Any validation error indicates invalid proposal - log and reject
                error!(
                    height = %parts.height,
                    round = %parts.round,
                    proposer = %parts.proposer,
                    error = ?error,
                    "Rejecting invalid proposal"
                );
                Ok(None)
            }
        }
    }

    /// Retrieves a decided block at the given height
    pub async fn get_decided_value(&self, height: Height) -> Option<DecidedValue> {
        self.store.get_decided_value(height).await.ok().flatten()
    }

    /// Stores a value with the given certificate without updating internal state or moving to the next height.
    pub async fn store_decided(
        &mut self,
        certificate: CommitCertificate<TestContext>,
    ) -> eyre::Result<()> {
        let (height, round, value_id) =
            (certificate.height, certificate.round, certificate.value_id);

        // Get the first proposal with the given value id. There may be multiple identical ones
        // if peers have restreamed at different rounds.
        let Ok(Some(proposal)) = self
            .store
            .get_undecided_proposal_by_value_id(value_id)
            .await
        else {
            return Err(eyre!(
                "Trying to store decided value with value id {value_id} at height {height} and round {round} for which there is no proposal"
            ));
        };

        self.store
            .store_decided_value(&certificate, proposal.value)
            .await?;

        Ok(())
    }

    /// Commits (or updates) a value with the given certificate, updating internal state
    /// and moving to the next height.
    pub async fn commit(
        &mut self,
        certificate: CommitCertificate<TestContext>,
    ) -> eyre::Result<()> {
        let (height, round, value_id) =
            (certificate.height, certificate.round, certificate.value_id);

        // Get the first proposal with the given value id. There may be multiple identical ones
        // if peers have restreamed at different rounds.
        let Ok(Some(proposal)) = self
            .store
            .get_undecided_proposal_by_value_id(value_id)
            .await
        else {
            return Err(eyre!(
                "Trying to commit a value with value id {value_id} at height {height} and round {round} for which there is no proposal"
            ));
        };

        let middleware = self.ctx.middleware();
        debug!(%height, %round, "Middleware: {middleware:?}");

        match middleware.on_commit(&self.ctx, &certificate, &proposal) {
            // Commit was successful, move to next height
            Ok(()) => {
                self.store
                    .store_decided_value(&certificate, proposal.value)
                    .await?;

                // Prune the store, keep the last HISTORY_LENGTH decided values, remove all undecided proposals for the decided height
                let retain_height = Height::new(height.as_u64().saturating_sub(HISTORY_LENGTH));
                self.store.prune(height, retain_height).await?;

                // Move to next height
                self.current_height = self.current_height.increment();
                self.current_round = Round::Nil;

                Ok(())
            }
            // Commit failed, reset height
            Err(e) => {
                error!("Middleware commit failed: {e}");
                Err(eyre!("Resetting at height {height}"))
            }
        }
    }

    pub async fn store_synced_value(
        &mut self,
        proposal: ProposedValue<TestContext>,
    ) -> eyre::Result<()> {
        self.store.store_undecided_proposal(proposal).await?;
        Ok(())
    }

    /// Retrieves a previously built proposal value for the given height and round.
    /// Called by the consensus engine to re-use a previously built value.
    /// There should be at most one proposal for a given height and round when the proposer is not byzantine.
    /// We assume this implementation is not byzantine and we are the proposer for the given height and round.
    /// Therefore there must be a single proposal for the rounds where we are the proposer, with the proposer address matching our own.
    pub async fn get_previously_built_value(
        &self,
        height: Height,
        round: Round,
    ) -> eyre::Result<Option<LocallyProposedValue<TestContext>>> {
        let proposals = self.store.get_undecided_proposals(height, round).await?;

        assert!(
            proposals.len() <= 1,
            "There should be at most one proposal for a given height and round"
        );

        proposals
            .first()
            .map(|p| LocallyProposedValue::new(p.height, p.round, p.value.clone()))
            .map(Some)
            .map(Ok)
            .unwrap_or_else(|| Ok(None))
    }

    /// Creates a new proposal value for the given height and round
    async fn create_proposal(
        &mut self,
        height: Height,
        round: Round,
    ) -> eyre::Result<ProposedValue<TestContext>> {
        assert_eq!(height, self.current_height);
        assert_eq!(round, self.current_round);

        // Create a new value
        let value = self.make_value();

        let proposal = ProposedValue {
            height,
            round,
            valid_round: Round::Nil,
            proposer: self.address, // We are the proposer
            value,
            validity: Validity::Valid, // Our proposals are de facto valid
        };

        // Insert the new proposal into the undecided proposals
        self.store
            .store_undecided_proposal(proposal.clone())
            .await?;

        Ok(proposal)
    }

    /// Make up a new value to propose
    /// A real application would have a more complex logic here,
    /// typically reaping transactions from a mempool and executing them against its state,
    /// before computing the merkle root of the new app state.
    fn make_value(&mut self) -> Value {
        let value = self.rng.gen_range(100..=100000);
        Value::new(value)
    }

    pub async fn get_proposal(
        &self,
        height: Height,
        round: Round,
        _valid_round: Round,
        _proposer: Address,
        value_id: ValueId,
    ) -> Option<LocallyProposedValue<TestContext>> {
        Some(LocallyProposedValue::new(
            height,
            round,
            Value::new(value_id.as_u64()),
        ))
    }

    /// Creates a new proposal value for the given height
    pub async fn propose_value(
        &mut self,
        height: Height,
        round: Round,
    ) -> eyre::Result<LocallyProposedValue<TestContext>> {
        assert_eq!(height, self.current_height);
        assert_eq!(round, self.current_round);

        let proposal = self.create_proposal(height, round).await?;

        Ok(LocallyProposedValue::new(
            proposal.height,
            proposal.round,
            proposal.value,
        ))
    }

    fn stream_id(&self) -> StreamId {
        let mut bytes = Vec::with_capacity(size_of::<u64>() + size_of::<u32>());
        bytes.extend_from_slice(&self.current_height.as_u64().to_be_bytes());
        bytes.extend_from_slice(&self.current_round.as_u32().unwrap().to_be_bytes());
        StreamId::new(bytes.into())
    }

    /// Creates a stream message containing a proposal part.
    /// Updates internal sequence number and current proposal.
    pub fn stream_proposal(
        &mut self,
        value: LocallyProposedValue<TestContext>,
        pol_round: Round,
    ) -> impl Iterator<Item = StreamMessage<ProposalPart>> {
        let parts = self.value_to_parts(value, pol_round);
        let stream_id = self.stream_id();

        let mut msgs = Vec::with_capacity(parts.len() + 1);
        let mut sequence = 0;

        for part in parts {
            let msg = StreamMessage::new(stream_id.clone(), sequence, StreamContent::Data(part));
            sequence += 1;
            msgs.push(msg);
        }

        msgs.push(StreamMessage::new(
            stream_id.clone(),
            sequence,
            StreamContent::Fin,
        ));

        msgs.into_iter()
    }

    fn value_to_parts(
        &self,
        value: LocallyProposedValue<TestContext>,
        pol_round: Round,
    ) -> Vec<ProposalPart> {
        let mut hasher = sha3::Keccak256::new();
        let mut parts = Vec::new();

        // Init
        // Include metadata about the proposal
        {
            parts.push(ProposalPart::Init(ProposalInit::new(
                value.height,
                value.round,
                pol_round,
                self.address,
            )));

            hasher.update(value.height.as_u64().to_be_bytes().as_slice());
            hasher.update(value.round.as_i64().to_be_bytes().as_slice());
        }

        // Data
        // Include each prime factor of the value as a separate proposal part
        {
            for factor in factor_value(value.value) {
                parts.push(ProposalPart::Data(ProposalData::new(factor)));

                hasher.update(factor.to_be_bytes().as_slice());
            }
        }

        // Fin
        // Sign the hash of the proposal parts
        {
            let hash = hasher.finalize().to_vec();
            let signature = self.signing_provider.sign(&hash);
            parts.push(ProposalPart::Fin(ProposalFin::new(signature)));
        }

        parts
    }

    /// Re-assemble a [`ProposedValue`] from its [`ProposalParts`].
    ///
    /// This is done by multiplying all the factors in the parts.
    ///
    /// ## Important
    /// This method assumes that the proposal parts have already been validated by `validate_proposal_parts`
    pub fn assemble_value_from_parts(
        parts: ProposalParts,
    ) -> eyre::Result<ProposedValue<TestContext>> {
        let init = parts.init().ok_or_else(|| eyre!("Missing Init part"))?;

        let value = parts
            .parts
            .iter()
            .filter_map(|part| part.as_data())
            .fold(1, |acc, data| acc * data.factor);

        Ok(ProposedValue {
            height: parts.height,
            round: parts.round,
            valid_round: init.pol_round,
            proposer: parts.proposer,
            value: Value::new(value),
            validity: Validity::Valid,
        })
    }
}

/// Encode a value to its byte representation
pub fn encode_value(value: &Value) -> Bytes {
    JsonCodec.encode(value).unwrap()
}

/// Decodes a Value from its byte representation
pub fn decode_value(bytes: Bytes) -> Option<Value> {
    JsonCodec.decode(bytes).ok()
}

/// Returns the list of prime factors of the given value
///
/// In a real application, this would typically split transactions
/// into chunks ino order to reduce bandwidth requirements due
/// to duplication of gossip messages.
fn factor_value(value: Value) -> Vec<u64> {
    let mut factors = Vec::new();
    let mut n = value.value;

    let mut i = 2;
    while i * i <= n {
        if n.is_multiple_of(i) {
            factors.push(i);
            n /= i;
        } else {
            i += 1;
        }
    }

    if n > 1 {
        factors.push(n);
    }

    factors
}
