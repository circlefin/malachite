use std::collections::HashSet;

use crate::streaming::{Buffer, Message};
use malachitebft_engine::util::streaming::Sequence;
use malachitebft_starknet_host::{proto::ProposalPart, streaming::MinHeap};

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
