//! Internal state of the application. This is a simplified abstract to keep it simple.
//! A regular application would have mempool implemented, a proper database and input methods like RPC.

use std::collections::HashSet;

use bytes::Bytes;
use eyre::eyre;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use sha3::Digest;
use tracing::{debug, error};

use malachitebft_app_channel::app::consensus::{ProposedValue, Role};
use malachitebft_app_channel::app::streaming::{StreamContent, StreamId, StreamMessage};
use malachitebft_app_channel::app::types::codec::Codec;
use malachitebft_app_channel::app::types::core::{CommitCertificate, Round, Validity};
use malachitebft_app_channel::app::types::{LocallyProposedValue, PeerId};
use malachitebft_test::codec::json::JsonCodec;
use malachitebft_test::{
    Address, Ed25519Provider, Genesis, Height, ProposalData, ProposalFin, ProposalInit,
    ProposalPart, TestContext, ValidatorSet, Value, ValueId,
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

    signing_provider: Ed25519Provider,
    streams_map: PartStreamsMap,
    rng: StdRng,
}

impl State {
    /// Creates a new State instance with the given validator address and starting height
    pub fn new(
        ctx: TestContext,
        config: Config,
        genesis: Genesis,
        address: Address,
        height: Height,
        store: Store,
        signing_provider: Ed25519Provider,
    ) -> Self {
        Self {
            ctx,
            config,
            genesis,
            address,
            store,
            signing_provider,
            current_height: height,
            current_round: Round::new(0),
            current_proposer: None,
            current_role: Role::None,
            streams_map: PartStreamsMap::new(),
            rng: StdRng::from_entropy(),
            peers: HashSet::new(),
        }
    }

    /// Returns the set of validators.
    pub fn get_validator_set(&self) -> &ValidatorSet {
        &self.genesis.validator_set
    }

    /// Returns the earliest height available in the state
    pub async fn get_earliest_height(&self) -> Height {
        self.store
            .min_decided_value_height()
            .await
            .unwrap_or_default()
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

        // Verify the proposal signature
        match self.verify_proposal_signature(&parts) {
            Ok(()) => {
                // Signature verified successfully, continue processing
            }
            Err(SignatureVerificationError::MissingInitPart) => {
                return Err(eyre!(
                    "Expected to have full proposal but `Init` proposal part is missing for proposer: {}",
                    parts.proposer
                ));
            }
            Err(SignatureVerificationError::MissingFinPart) => {
                return Err(eyre!(
                    "Expected to have full proposal but `Fin` proposal part is missing for proposer: {}",
                    parts.proposer
                ));
            }
            Err(SignatureVerificationError::ProposerNotFound) => {
                error!(proposer = %parts.proposer, "Proposer not found in validator set");
                return Ok(None);
            }
            Err(SignatureVerificationError::InvalidSignature) => {
                error!(proposer = %parts.proposer, "Invalid signature in Fin part");
                return Ok(None);
            }
        }

        let proposal_height = parts.height;

        // Re-assemble the proposal from its parts
        let value = assemble_value_from_parts(parts)?;

        if proposal_height == self.current_height {
            self.store.store_undecided_proposal(value.clone()).await?;
        } else {
            self.store.store_pending_proposal(value.clone()).await?;
        }

        Ok(Some(value))
    }

    /// Retrieves a decided block at the given height
    pub async fn get_decided_value(&self, height: Height) -> Option<DecidedValue> {
        self.store.get_decided_value(height).await.ok().flatten()
    }

    /// Commits a value with the given certificate, updating internal state
    /// and moving to the next height
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

    /// Verifies the signature of the proposal.
    /// Returns `Ok(())` if the signature is valid, or an appropriate `SignatureVerificationError`.
    fn verify_proposal_signature(
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

        // Retrieve the the proposer
        let proposer = self
            .get_validator_set()
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
}

/// Re-assemble a [`ProposedValue`] from its [`ProposalParts`].
///
/// This is done by multiplying all the factors in the parts.
fn assemble_value_from_parts(parts: ProposalParts) -> eyre::Result<ProposedValue<TestContext>> {
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
        validity: Validity::Valid, // TODO: Check signature in Fin part
    })
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
        if n % i == 0 {
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

/// Represents errors that can occur during the verification of a proposal's signature.
#[derive(Debug)]
enum SignatureVerificationError {
    /// Indicates that the `Init` part of the proposal is unexpectedly missing.
    MissingInitPart,

    /// Indicates that the `Fin` part of the proposal is unexpectedly missing.
    MissingFinPart,

    /// Indicates that the proposer was not found in the validator set.
    ProposerNotFound,

    /// Indicates that the signature in the `Fin` part is invalid.
    InvalidSignature,
}
