//! Internal state of the application. This is a simplified abstract to keep it simple.
//! A regular application would have mempool implemented, a proper database and input methods like RPC.

use std::collections::HashMap;

use bytes::Bytes;
use tracing::error;

use malachite_app_channel::app::consensus::ProposedValue;
use malachite_app_channel::app::host::LocallyProposedValue;
use malachite_app_channel::app::streaming::{StreamContent, StreamMessage};
use malachite_app_channel::app::types::codec::Codec;
use malachite_app_channel::app::types::core::{CommitCertificate, Round, Validity};
use malachite_app_channel::app::types::sync::DecidedValue;
use malachite_test::codec::proto::ProtobufCodec;
use malachite_test::{Address, BlockMetadata, Content, Height, ProposalPart, TestContext, Value};

/// Decodes a Value from its byte representation using ProtobufCodec
pub fn decode_value(bytes: Bytes) -> Value {
    ProtobufCodec.decode(bytes).unwrap()
}

/// Encodes a Value into its byte representation using ProtobufCodec
pub fn encode_value(value: &Value) -> Bytes {
    ProtobufCodec.encode(value).unwrap()
}

/// Represents the internal state of the application node
/// Contains information about current height, round, proposals and blocks
pub struct State {
    pub current_height: Height,
    pub current_round: Round,
    pub current_proposer: Option<Address>,

    earliest_height: Height,
    address: Address,
    sequence: u64,
    undecided_proposals: HashMap<Height, ProposedValue<TestContext>>,
    decided_proposals: HashMap<Height, ProposedValue<TestContext>>,
    blocks: HashMap<Height, DecidedValue<TestContext>>,
    current_proposal: Option<StreamMessage<ProposalPart>>,
}

impl State {
    /// Creates a new State instance with the given validator address and starting height
    pub fn new(address: Address, height: Height) -> Self {
        Self {
            earliest_height: height,
            current_height: height,
            current_round: Round::new(0),
            current_proposer: None,
            address,
            sequence: 0,
            undecided_proposals: HashMap::new(),
            decided_proposals: HashMap::new(),
            blocks: HashMap::new(),
            current_proposal: None,
        }
    }

    /// Returns the earliest height available in the state
    pub fn get_earliest_height(&self) -> Height {
        self.earliest_height
    }

    /// Processes and adds a new proposal to the state if it's valid
    /// Returns Some(ProposedValue) if the proposal was accepted, None otherwise
    pub fn add_proposal(
        &mut self,
        stream_message: StreamMessage<ProposalPart>,
    ) -> Option<ProposedValue<TestContext>> {
        let StreamContent::Data(proposal_part) = stream_message.content else {
            error!("Invalid proposal: {:?}", stream_message.content);
            return None;
        };

        if proposal_part.height > self.current_height
            || proposal_part.height == self.current_height
                && proposal_part.round >= self.current_round
        {
            assert!(proposal_part.fin); // we only implemented 1 part === 1 proposal

            let value = proposal_part.content.metadata.value();

            let proposal = ProposedValue {
                height: proposal_part.height,
                round: proposal_part.round,
                valid_round: Round::Nil,
                validator_address: proposal_part.validator_address,
                value,
                validity: Validity::Valid,
                extension: None,
            };

            self.undecided_proposals
                .insert(proposal_part.height, proposal.clone());

            Some(proposal)
        } else {
            None
        }
    }

    /// Retrieves a decided block at the given height
    pub fn get_decided_value(&self, height: &Height) -> Option<&DecidedValue<TestContext>> {
        self.blocks.get(height)
    }

    /// Commits a block with the given certificate, updating internal state
    /// and moving to the next height
    pub fn commit_block(&mut self, certificate: CommitCertificate<TestContext>) {
        // Sort out proposals
        for (height, value) in self.undecided_proposals.clone() {
            if height > self.current_height {
                continue;
            }

            if height == certificate.height {
                self.decided_proposals.insert(height, value);
            }

            self.undecided_proposals.remove(&height);
        }

        // Commit block transactions to "database"
        // TODO: retrieve all transactions from block parts
        let value = self.decided_proposals.get(&certificate.height).unwrap();
        let value_bytes = encode_value(&value.value);

        self.blocks.insert(
            self.current_height,
            DecidedValue {
                value_bytes,
                certificate,
            },
        );

        // Move to next height
        self.current_height = self.current_height.increment();
        self.current_round = Round::new(0);
    }

    /// Retrieves a previously built proposal value for the given height
    pub fn get_previously_built_value(
        &self,
        height: &Height,
    ) -> Option<&ProposedValue<TestContext>> {
        self.undecided_proposals.get(height)
    }

    /// Creates a new proposal value for the given height
    /// Returns either a previously built proposal or creates a new one
    pub fn propose_value(&mut self, height: &Height) -> ProposedValue<TestContext> {
        if let Some(proposal) = self.get_previously_built_value(height) {
            proposal.clone()
        } else {
            assert_eq!(height.as_u64(), self.current_height.as_u64());

            // We create a new value.
            let value = Value::new(42); // TODO: get value

            let proposal = ProposedValue {
                height: *height,
                round: self.current_round,
                valid_round: Round::Nil,
                validator_address: self.address,
                value,
                validity: Validity::Valid,
                extension: None,
            };

            // Insert the new proposal into the undecided proposals.
            self.undecided_proposals.insert(*height, proposal.clone());

            proposal
        }
    }

    /// Creates a broadcast message containing a proposal part
    /// Updates internal sequence number and current proposal
    pub fn create_broadcast_message(
        &mut self,
        value: LocallyProposedValue<TestContext>,
    ) -> StreamMessage<ProposalPart> {
        // TODO: create proof properly.
        let fake_proof = [
            self.current_height.as_u64().to_le_bytes().to_vec(),
            self.current_round.as_u32().unwrap().to_le_bytes().to_vec(),
        ]
        .concat();

        let content = Content::new(&BlockMetadata::new(fake_proof.into(), value.value));

        let proposal_part = ProposalPart::new(
            self.current_height,
            self.current_round,
            self.sequence,
            self.address,
            content,
            true, // each proposal part is a full proposal
        );

        let stream_content = StreamContent::Data(proposal_part);
        let msg = StreamMessage::new(self.sequence, self.sequence, stream_content);

        self.sequence += 1;
        self.current_proposal = Some(msg.clone());

        msg
    }
}
