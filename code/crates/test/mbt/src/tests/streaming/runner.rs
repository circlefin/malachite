use super::utils;
use crate::streaming::{MessageType, State as SpecificationState};
use itf::Runner as ItfRunner;
use malachitebft_engine::util::streaming::{StreamContent, StreamMessage};
use malachitebft_peer::PeerId;
use malachitebft_starknet_host::{
    streaming::{PartStreamsMap, StreamState as StreamStateImpl},
    types::ProposalPart,
};

pub struct StreamingRunner {
    peer_id: PeerId,
    stream_id: u64,
}

impl StreamingRunner {
    pub fn new(peer_id: PeerId, stream_id: u64) -> Self {
        Self { peer_id, stream_id }
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
            buffer: utils::spec_to_impl_buffer(&expected.state.buffer, self.stream_id),
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
            .insert((self.peer_id, self.stream_id), initial_state);
        Ok(streams_map)
    }

    fn step(
        &mut self,
        actual: &mut Self::ActualState,
        expected: &Self::ExpectedState,
    ) -> Result<Self::Result, Self::Error> {
        let stream_state = actual.streams.get(&(self.peer_id, self.stream_id)).unwrap();

        println!("🔸 step: actual state={:?}", stream_state);
        println!("🔸 step: model input={:?}", expected.incoming_message);
        println!("🔸 step: model state={:?}", expected.state);

        let message = match &expected.incoming_message {
            Some(msg) => match &msg.msg_type {
                MessageType::Init => {
                    let proposal_init = utils::generate_dummy_proposal_init();
                    StreamMessage::<ProposalPart>::new(
                        self.stream_id,
                        msg.sequence as u64,
                        StreamContent::Data(ProposalPart::Init(proposal_init)),
                    )
                }
                MessageType::Data => {
                    let transactions = utils::generate_dummy_transactions();
                    StreamMessage::<ProposalPart>::new(
                        self.stream_id,
                        msg.sequence as u64,
                        StreamContent::Data(ProposalPart::Transactions(transactions)),
                    )
                }
                MessageType::Fin => {
                    //Q: StreamContent can be Data or Fin, but also ProposalPart has Fin variant
                    // When will ProposalPart::Fin be used?
                    StreamMessage::<ProposalPart>::new(
                        self.stream_id,
                        msg.sequence as u64,
                        StreamContent::Fin(true),
                    )
                }
            },
            None => {
                return Ok(());
            }
        };

        actual.insert(self.peer_id, message);

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
        let stream_state = actual.streams.get(&(self.peer_id, self.stream_id));

        match stream_state {
            Some(stream_state) => {
                println!("🟢 state invariant: actual state={:?}", stream_state);
                println!("🟢 state invariant: expected state={:?}", expected.state);

                assert!(
                    utils::compare_buffers(&stream_state.buffer, &expected.state.buffer),
                    "unexpected buffer value"
                );

                assert_eq!(
                    stream_state.init_info.is_some(),
                    expected.state.init_message.is_some(),
                    "unexpected init info value"
                );

                assert!(
                    utils::messages_equal_sequences(
                        &stream_state.seen_sequences,
                        &expected.state.received
                    ),
                    "unexpected seen sequences value"
                );

                assert_eq!(
                    stream_state.next_sequence, expected.state.next_sequence as u64,
                    "unexpected next sequence value"
                );

                assert_eq!(
                    stream_state.total_messages as i32, expected.state.total_messages,
                    "unexpected total messages value"
                );

                assert_eq!(
                    stream_state.fin_received, expected.state.fin_received,
                    "unexpected fin received value"
                );

                assert_eq!(
                    stream_state.emitted_messages,
                    expected.state.emitted.len(),
                    "unexpected emitted messages value"
                );

                Ok(true)
            }
            None => {
                // This means message is emitted completely, thus stream (StreamState) is
                //  removed from streams map
                if expected.state.total_messages != 0
                    && expected.state.total_messages == expected.state.emitted.len() as i32
                {
                    return Ok(true);
                } else {
                    return Ok(false);
                }
            }
        }
    }
}
