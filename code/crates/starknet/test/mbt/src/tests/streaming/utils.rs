use std::collections::HashSet;

use crate::streaming::{Buffer, Message};
use malachitebft_core_types::Round;
use malachitebft_engine::util::streaming::{Sequence, StreamId};
use malachitebft_engine::util::streaming::{StreamContent, StreamMessage};
use malachitebft_starknet_host::types::{PrivateKey, TransactionBatch};
use malachitebft_starknet_host::{
    streaming::MinHeap,
    types::{Address, Height, ProposalInit, ProposalPart, Transaction},
};

pub fn messages_equal_sequences(
    sequences: &HashSet<Sequence>,
    messages: &HashSet<Message>,
) -> bool {
    messages
        .iter()
        .map(|msg| msg.sequence as u64)
        .collect::<HashSet<_>>()
        == *sequences
}

//Because both buffers use same BinaryHeap implementation, we can assume that the order of elements
//will be the same for the same set of elements thus we can just compare the sets of sequences
pub fn compare_buffers(actual_buffer: &MinHeap<ProposalPart>, expected_buffer: &Buffer) -> bool {
    let actual_set: HashSet<_> = actual_buffer
        .0
        .iter()
        .map(|msg| msg.0.sequence as i64)
        .collect();
    let expected_set: HashSet<_> = expected_buffer.0.iter().map(|rec| rec.0).collect();

    actual_set == expected_set
}

pub fn spec_to_impl_buffer(spec_buffer: &Buffer, stream_id: StreamId) -> MinHeap<ProposalPart> {
    let mut impl_buffer = MinHeap::default();

    for rec in &spec_buffer.0 {
        let message = match rec.1.msg_type {
            crate::streaming::MessageType::Init => {
                let proposal_init = generate_dummy_proposal_init();
                StreamMessage::<ProposalPart>::new(
                    stream_id.clone(),
                    rec.0 as u64,
                    StreamContent::Data(ProposalPart::Init(proposal_init)),
                )
            }
            crate::streaming::MessageType::Data => {
                let transactions = generate_dummy_transactions();
                StreamMessage::<ProposalPart>::new(
                    stream_id.clone(),
                    rec.0 as u64,
                    StreamContent::Data(ProposalPart::Transactions(transactions)),
                )
            }
            crate::streaming::MessageType::Fin => StreamMessage::<ProposalPart>::new(
                stream_id.clone(),
                rec.0 as u64,
                StreamContent::Fin,
            ),
        };
        impl_buffer.push(message);
    }

    impl_buffer
}

// Specifications init messages is just string, so no useful data can be extracted from it
pub fn init_message_to_proposal_init(message: &Option<Message>) -> Option<ProposalInit> {
    message.as_ref().map(|_| generate_dummy_proposal_init())
}

pub fn generate_dummy_proposal_init() -> ProposalInit {
    let bytes: [u8; 32] = [
        0x08, 0xd7, 0xa2, 0x0f, 0x32, 0x0c, 0x4d, 0x23, 0xd9, 0xad, 0xf3, 0x29, 0xf5, 0x7c, 0x7b,
        0x62, 0x35, 0x9d, 0x97, 0xce, 0x0b, 0xb3, 0xb7, 0x66, 0x19, 0xd8, 0x50, 0x4d, 0x59, 0xa1,
        0x88, 0x4b,
    ];
    let private_key = PrivateKey::from(bytes);

    let proposer_addr = Address::new(*private_key.public_key().as_bytes());

    let height = Height {
        block_number: 1,
        fork_id: 1,
    };

    let round = Round::new(2);
    let valid_round = Round::new(1);

    ProposalInit {
        height,
        round,
        valid_round,
        proposer: proposer_addr,
    }
}

pub fn generate_dummy_transactions() -> TransactionBatch {
    let tx1 = Transaction::new(vec![0x01, 0x02, 0x03]);
    let tx2 = Transaction::new(vec![0x04, 0x05, 0x06]);
    let tx3 = Transaction::new(vec![0x07, 0x08, 0x09]);

    TransactionBatch::new(vec![tx1, tx2, tx3])
}
