use super::utils;
use crate::streaming::{MessageType, State as SpecificationState};
use itf::Runner as ItfRunner;
use malachitebft_core_types::Round;
use malachitebft_engine::util::streaming::{StreamContent, StreamMessage};
use malachitebft_peer::PeerId;
use malachitebft_starknet_host::{
    streaming::{PartStreamsMap, StreamState as StreamStateImpl},
    types::{Address, Height, ProposalInit, ProposalPart, Transaction, Transactions},
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
        streams_map
            .streams
            .insert((self.peer_id, self.stream_id), StreamStateImpl::default());
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
                    // Dummy proposer address
                    let bytes: [u8; 32] = [
                        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C,
                        0x0D, 0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18,
                        0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F, 0x20,
                    ];
                    let proposer_addr = Address::new(bytes);

                    let height = Height {
                        block_number: 1,
                        fork_id: 1,
                    };

                    let round = Round::new(2);
                    let valid_round = Round::new(1);

                    let proposal_init = ProposalInit {
                        height: height,
                        proposal_round: round,
                        valid_round: valid_round,
                        proposer: proposer_addr,
                    };

                    StreamMessage::<ProposalPart>::new(
                        self.stream_id,
                        msg.sequence as u64,
                        StreamContent::Data(ProposalPart::Init(proposal_init)),
                    )
                    // actual.insert(self.peer_id, message);
                }
                MessageType::Data => {
                    // Dummy transactions
                    let tx1 = Transaction::new(vec![0x01, 0x02, 0x03]);
                    let tx2 = Transaction::new(vec![0x04, 0x05, 0x06]);
                    let tx3 = Transaction::new(vec![0x07, 0x08, 0x09]);

                    let tx_vec = vec![tx1, tx2, tx3];

                    let transactions = Transactions::new(tx_vec);
                    StreamMessage::<ProposalPart>::new(
                        self.stream_id,
                        msg.sequence as u64,
                        StreamContent::Data(ProposalPart::Transactions(transactions)),
                    )
                    // actual.insert(self.peer_id, message);
                }
                MessageType::Fin => {
                    //Q: StreamContent can be Data or Fin, but also ProposalPart has Fin variant
                    // When will ProposalPart::Fin be used?
                    StreamMessage::<ProposalPart>::new(
                        self.stream_id,
                        msg.sequence as u64,
                        StreamContent::Fin(true),
                    )
                    // actual.insert(self.peer_id, message);
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
