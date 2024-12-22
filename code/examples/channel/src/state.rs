//! Internal state of the application. This is a simplified abstract to keep it simple.
//! A regular application would have mempool implemented, a proper database and input methods like RPC.

use std::collections::{BTreeMap, HashMap, HashSet};

use bytes::Bytes;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use sha3::Digest;
use tracing::debug;

use malachitebft_app_channel::app::consensus::ProposedValue;
use malachitebft_app_channel::app::host::LocallyProposedValue;
use malachitebft_app_channel::app::streaming::{StreamContent, StreamMessage};
use malachitebft_app_channel::app::types::codec::Codec;
use malachitebft_app_channel::app::types::core::{CommitCertificate, Round, Validity};
use malachitebft_app_channel::app::types::sync::DecidedValue;
use malachitebft_app_channel::app::types::PeerId;
use malachitebft_test::codec::proto::ProtobufCodec;
use malachitebft_test::{
    Address, Height, ProposalData, ProposalFin, ProposalInit, ProposalPart, TestContext, Value,
};

use crate::streaming::{PartStreamsMap, ProposalParts};

/// Represents the internal state of the application node
/// Contains information about current height, round, proposals and blocks
pub struct State {
    ctx: TestContext,
    address: Address,

    pub current_height: Height,
    pub current_round: Round,
    pub current_proposer: Option<Address>,

    undecided_proposals: HashMap<(Height, Round), ProposedValue<TestContext>>,
    decided_proposals: HashMap<Height, ProposedValue<TestContext>>,
    decided_values: BTreeMap<Height, DecidedValue<TestContext>>,

    stream_id: u64,
    streams_map: PartStreamsMap,

    rng: StdRng,
    pub peers: HashSet<PeerId>,
}

// Make up a seed for the rng based on our address in
// order for each node to likely propose different values at
// each round.
fn seed_from_address(address: &Address) -> u64 {
    address.into_inner().chunks(8).fold(0u64, |acc, chunk| {
        let term = chunk.iter().fold(0u64, |acc, &x| {
            acc.wrapping_shl(8).wrapping_add(u64::from(x))
        });
        acc.wrapping_add(term)
    })
}

impl State {
    /// Creates a new State instance with the given validator address and starting height
    pub fn new(ctx: TestContext, address: Address, height: Height) -> Self {
        Self {
            ctx,
            current_height: height,
            current_round: Round::new(0),
            current_proposer: None,
            address,
            stream_id: 0,
            undecided_proposals: HashMap::new(),
            decided_proposals: HashMap::new(),
            decided_values: BTreeMap::new(),
            streams_map: PartStreamsMap::new(),
            rng: StdRng::seed_from_u64(seed_from_address(&address)),
            peers: HashSet::new(),
        }
    }

    /// Returns the earliest height available in the state
    pub fn get_earliest_height(&self) -> Height {
        self.decided_values
            .keys()
            .next()
            .copied()
            .unwrap_or_default()
    }

    /// Processes and adds a new proposal to the state if it's valid
    /// Returns Some(ProposedValue) if the proposal was accepted, None otherwise
    pub fn received_proposal_part(
        &mut self,
        from: PeerId,
        part: StreamMessage<ProposalPart>,
    ) -> Option<ProposedValue<TestContext>> {
        let sequence = part.sequence;

        // Check if we have a full proposal
        let parts = self.streams_map.insert(from, part)?;

        // Check if the proposal is outdated
        if parts.height < self.current_height {
            debug!(
                height = %self.current_height,
                round = %self.current_round,
                part.height = %parts.height,
                part.round = %parts.round,
                part.sequence = %sequence,
                "Received outdated proposal part, ignoring"
            );

            return None;
        }

        // Re-assemble the proposal from its parts
        let value = assemble_value_from_parts(parts);

        self.undecided_proposals
            .insert((value.height, value.round), value.clone());

        Some(value)
    }

    /// Retrieves a decided block at the given height
    pub fn get_decided_value(&self, height: &Height) -> Option<&DecidedValue<TestContext>> {
        self.decided_values.get(height)
    }

    /// Commits a value with the given certificate, updating internal state
    /// and moving to the next height
    pub fn commit(&mut self, certificate: CommitCertificate<TestContext>) {
        // Sort out proposals
        for ((height, round), value) in self.undecided_proposals.clone() {
            if height > self.current_height {
                continue;
            }

            if height == certificate.height {
                self.decided_proposals.insert(height, value);
            }

            self.undecided_proposals.remove(&(height, round));
        }

        let value = self.decided_proposals.get(&certificate.height).unwrap();
        let value_bytes = encode_value(&value.value);

        self.decided_values.insert(
            self.current_height,
            DecidedValue::new(value_bytes, certificate),
        );

        // Move to next height
        self.current_height = self.current_height.increment();
        self.current_round = Round::new(0);
    }

    /// Retrieves a previously built proposal value for the given height
    pub fn get_previously_built_value(
        &self,
        height: Height,
        round: Round,
    ) -> Option<LocallyProposedValue<TestContext>> {
        let proposal = self.undecided_proposals.get(&(height, round))?;

        Some(LocallyProposedValue::new(
            proposal.height,
            proposal.round,
            proposal.value,
            proposal.extension.clone(),
        ))
    }

    /// Creates a new proposal value for the given height
    /// Returns either a previously built proposal or creates a new one
    fn create_proposal(&mut self, height: Height, round: Round) -> ProposedValue<TestContext> {
        assert_eq!(height, self.current_height);
        assert_eq!(round, self.current_round);

        // We create a new value.
        let value = self.make_value();

        let proposal = ProposedValue {
            height,
            round,
            valid_round: Round::Nil,
            proposer: self.address, // We are the proposer
            value,
            validity: Validity::Valid, // Our proposals are de facto valid
            extension: None,           // Vote extension can be added here
        };

        // Insert the new proposal into the undecided proposals.
        self.undecided_proposals
            .insert((height, round), proposal.clone());

        proposal
    }

    /// Make up a new value to propose
    /// A real application would have a more complex logic here,
    /// typically reaping transactions from a mempool and executing them against its state,
    /// before computing the merkle root of the new app state.
    fn make_value(&mut self) -> Value {
        let value = self.rng.gen_range(100..=100000);
        Value::new(value)
    }

    /// Creates a new proposal value for the given height
    /// Returns either a previously built proposal or creates a new one
    pub fn propose_value(
        &mut self,
        height: Height,
        round: Round,
    ) -> LocallyProposedValue<TestContext> {
        assert_eq!(height, self.current_height);
        assert_eq!(round, self.current_round);

        let proposal = self.create_proposal(height, round);

        LocallyProposedValue::new(
            proposal.height,
            proposal.round,
            proposal.value,
            proposal.extension,
        )
    }

    /// Creates a stream message containing a proposal part.
    /// Updates internal sequence number and current proposal.
    pub fn stream_proposal(
        &mut self,
        value: LocallyProposedValue<TestContext>,
    ) -> impl Iterator<Item = StreamMessage<ProposalPart>> {
        let parts = self.value_to_parts(value);

        let stream_id = self.stream_id;
        self.stream_id += 1;

        let mut msgs = Vec::with_capacity(parts.len() + 1);
        let mut sequence = 0;

        for part in parts {
            let msg = StreamMessage::new(stream_id, sequence, StreamContent::Data(part));
            sequence += 1;
            msgs.push(msg);
        }

        msgs.push(StreamMessage::new(
            stream_id,
            sequence,
            StreamContent::Fin(true),
        ));

        msgs.into_iter()
    }

    fn value_to_parts(&self, value: LocallyProposedValue<TestContext>) -> Vec<ProposalPart> {
        let mut hasher = sha3::Keccak256::new();
        let mut parts = Vec::new();

        // Init
        // Include metadata about the proposal
        {
            parts.push(ProposalPart::Init(ProposalInit::new(
                value.height,
                value.round,
                self.address,
            )));

            hasher.update(value.height.as_u64().to_be_bytes().as_slice());
            hasher.update(value.round.as_i64().to_be_bytes().as_slice());

            if let Some(ext) = &value.extension {
                hasher.update(ext.data.as_ref());
            }
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
            let signature = self.ctx.signing_provider.sign(&hash);
            parts.push(ProposalPart::Fin(ProposalFin::new(signature)));
        }

        parts
    }
}

/// Re-assemble a [`ProposedValue`] from its [`ProposalParts`].
///
/// This is done by multiplying all the factors in the parts.
fn assemble_value_from_parts(parts: ProposalParts) -> ProposedValue<TestContext> {
    let value = parts
        .parts
        .iter()
        .filter_map(|part| part.as_data())
        .fold(1, |acc, data| acc * data.factor);

    ProposedValue {
        height: parts.height,
        round: parts.round,
        valid_round: Round::Nil,
        proposer: parts.proposer,
        value: Value::new(value),
        validity: Validity::Valid, // TODO: Check signature in Fin part
        extension: None,
    }
}

/// Decodes a Value from its byte representation using ProtobufCodec
pub fn decode_value(bytes: Bytes) -> Value {
    ProtobufCodec.decode(bytes).unwrap()
}

/// Encodes a Value into its byte representation using ProtobufCodec
pub fn encode_value(value: &Value) -> Bytes {
    ProtobufCodec.encode(value).unwrap()
}

/// Returns the list of prime factors of the given value
///
/// In a real application, this would typically split transactions
/// into chunks ino order to reduce bandwidth requirements due
/// to duplication of gossip messages.
fn factor_value(value: Value) -> Vec<u64> {
    let mut factors = Vec::new();
    let mut n = value.as_u64();

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
