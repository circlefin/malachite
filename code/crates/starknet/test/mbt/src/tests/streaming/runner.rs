use std::collections::HashSet;

use super::utils;
use crate::streaming::{Message, MessageType, State as SpecificationState};
use itf::Runner as ItfRunner;
use malachitebft_engine::util::streaming::{StreamContent, StreamId, StreamMessage};
use malachitebft_peer::PeerId;
use malachitebft_starknet_host::{
    streaming::{PartStreamsMap, StreamState as StreamStateImpl},
    types::ProposalPart,
};

pub struct StreamingRunner {
    peer_id: PeerId,
    stream_id: StreamId,
    incoming_messages_pool: HashSet<Message>,
    complete_proposal_message_sequence: Vec<Message>,
}

impl StreamingRunner {
    pub fn new(peer_id: PeerId, stream_id: StreamId) -> Self {
        let complete_proposal_message_sequence = vec![
            Message {
                sequence: 0,
                msg_type: MessageType::Init,
                payload: "Init".to_string(),
            },
            Message {
                sequence: 1,
                msg_type: MessageType::Data,
                payload: "Data 1".to_string(),
            },
            Message {
                sequence: 2,
                msg_type: MessageType::Data,
                payload: "Data 2".to_string(),
            },
            Message {
                sequence: 3,
                msg_type: MessageType::Fin,
                payload: "Fin".to_string(),
            },
        ];
        let incoming_messages_pool = complete_proposal_message_sequence.iter().cloned().collect();
        Self {
            peer_id,
            stream_id,
            incoming_messages_pool,
            complete_proposal_message_sequence,
        }
    }
}

impl ItfRunner for StreamingRunner {
    type ActualState = PartStreamsMap;

    // There is no result in the model, so it is empty
    type Result = ();

    type ExpectedState = SpecificationState;

    type Error = ();

    fn init(&mut self, expected: &Self::ExpectedState) -> Result<Self::ActualState, Self::Error> {
        println!("🔵 init: expected state={:?}", expected.state);
        let mut streams_map = PartStreamsMap::default();

        let initial_state: StreamStateImpl<ProposalPart> = StreamStateImpl {
            buffer: utils::spec_to_impl_buffer(&expected.state.buffer, self.stream_id.clone()),
            init_info: utils::init_message_to_proposal_init(&expected.incoming_message),
            seen_sequences: expected
                .state
                .received
                .iter()
                .map(|msg| msg.sequence as u64)
                .collect(),
            next_sequence: expected.state.next_sequence as u64,
            total_messages: expected.state.total_messages as usize,
            fin_received: expected.state.fin_received,
            emitted_messages: expected.state.emitted.len(),
        };

        streams_map
            .streams
            .insert((self.peer_id, self.stream_id.clone()), initial_state);
        Ok(streams_map)
    }

    fn step(
        &mut self,
        actual: &mut Self::ActualState,
        expected: &Self::ExpectedState,
    ) -> Result<Self::Result, Self::Error> {
        let stream_state = actual.streams.get(&(self.peer_id, self.stream_id.clone()));
        // If exact stream state can't be found, then the proposal is completely emitted and
        // stream is already removed
        println!("🔸 step: model input={:?}", expected.incoming_message);
        match stream_state {
            Some(stream_state) => {
                println!("🔸 step: actual state={stream_state:?}");
                println!("🔸 step: model state={:?}", expected.state);

                let message = match &expected.incoming_message {
                    Some(msg) => match &msg.msg_type {
                        MessageType::Init => {
                            let proposal_init = utils::generate_dummy_proposal_init();
                            StreamMessage::<ProposalPart>::new(
                                self.stream_id.clone(),
                                msg.sequence as u64,
                                StreamContent::Data(ProposalPart::Init(proposal_init)),
                            )
                        }
                        MessageType::Data => {
                            let transactions = utils::generate_dummy_transactions();
                            StreamMessage::<ProposalPart>::new(
                                self.stream_id.clone(),
                                msg.sequence as u64,
                                StreamContent::Data(ProposalPart::Transactions(transactions)),
                            )
                        }
                        MessageType::Fin => StreamMessage::<ProposalPart>::new(
                            self.stream_id.clone(),
                            msg.sequence as u64,
                            StreamContent::Fin,
                        ),
                    },
                    None => {
                        return Ok(());
                    }
                };

                actual.insert(self.peer_id, message);
            }

            None => println!("🔸 stream state not found (proposal emitted completely)"),
        }

        Ok(())
    }

    // If there is no result, then the result invariant is always true
    fn result_invariant(
        &self,
        _result: &Self::Result,
        _expected: &Self::ExpectedState,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }

    fn state_invariant(
        &self,
        actual: &Self::ActualState,
        expected: &Self::ExpectedState,
    ) -> Result<bool, Self::Error> {
        let actual_stream_state = actual.streams.get(&(self.peer_id, self.stream_id.clone()));

        match actual_stream_state {
            Some(actual_stream_state) => {
                println!("🟢 state invariant: actual state={actual_stream_state:?}");
                println!("🟢 state invariant: expected state={:?}", expected.state);

                // Compare the actual and expected states
                assert!(
                    utils::compare_buffers(&actual_stream_state.buffer, &expected.state.buffer),
                    "unexpected buffer value"
                );

                assert_eq!(
                    actual_stream_state.init_info.is_some(),
                    expected.state.init_message.is_some(),
                    "unexpected init info value"
                );

                assert!(
                    utils::messages_equal_sequences(
                        &actual_stream_state.seen_sequences,
                        &expected.state.received
                    ),
                    "unexpected seen sequences value"
                );

                assert_eq!(
                    actual_stream_state.next_sequence, expected.state.next_sequence as u64,
                    "unexpected next sequence value"
                );

                assert_eq!(
                    actual_stream_state.total_messages as i32, expected.state.total_messages,
                    "unexpected total messages value"
                );

                assert_eq!(
                    actual_stream_state.fin_received, expected.state.fin_received,
                    "unexpected fin received value"
                );

                assert_eq!(
                    actual_stream_state.emitted_messages,
                    expected.state.emitted.len(),
                    "unexpected emitted messages value"
                );

                // Check if invariant is satisfied
                if actual_stream_state.fin_received {
                    assert!(
                        actual_stream_state.total_messages > 0,
                        "total messages equal to 0 after fin received"
                    );

                    assert!(
                        actual_stream_state.next_sequence
                            <= actual_stream_state.total_messages as u64,
                        "next sequence greater than total messages after fin received"
                    );
                }

                assert!(
                    actual_stream_state.seen_sequences.is_subset(
                        &self
                            .incoming_messages_pool
                            .iter()
                            .map(|msg| msg.sequence as u64)
                            .collect()
                    ),
                    "seen sequences are not subset of incoming messages pool"
                );

                assert!(
                    actual_stream_state.emitted_messages
                        <= self.complete_proposal_message_sequence.len(),
                    "emitted messages length exceeds the complete proposal message sequence length"
                );

                Ok(true)
            }
            None => {
                // This means message is emitted completely, thus stream (StreamState) is
                //  removed from streams map
                if expected.state.init_message.is_some()
                    && expected.state.fin_received
                    && expected.state.received == self.incoming_messages_pool
                    && expected.state.emitted == self.complete_proposal_message_sequence
                    && expected.state.emitted.len() as i32 == expected.state.total_messages
                {
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
        }
    }
}
