use super::utils;
use crate::streaming::State as StateQuint;
use itf::Runner as ItfRunner;
use malachitebft_starknet_host::{proto::ProposalPart, streaming::StreamState as StreamStateImpl};
pub struct StreamingRunner {}

impl ItfRunner for StreamingRunner {
    //TODO: Rename State names?
    type ActualState = StreamStateImpl<ProposalPart>;
    //TODO: Check if this is right to be Result
    type Result = StreamStateImpl<ProposalPart>;

    type ExpectedState = StateQuint;

    type Error = ();

    fn init(&mut self, expected: &Self::ExpectedState) -> Result<Self::ActualState, Self::Error> {
        println!("🔵 init: expected state={:?}", expected.state);

        Ok(StreamStateImpl::default())
    }

    fn step(
        &mut self,
        actual: &mut Self::ActualState,
        expected: &Self::ExpectedState,
    ) -> Result<Self::Result, Self::Error> {
        todo!()
    }

    fn result_invariant(
        &self,
        result: &Self::Result,
        expected: &Self::ExpectedState,
    ) -> Result<bool, Self::Error> {
        todo!()
    }

    fn state_invariant(
        &self,
        actual: &Self::ActualState,
        expected: &Self::ExpectedState,
    ) -> Result<bool, Self::Error> {
        println!("🟢 state invariant: actual state={:?}", actual);
        println!("🟢 state invariant: expected state={:?}", expected.state);

        assert!(
            utils::compare_buffers(&actual.buffer, &expected.state.buffer),
            "unexpected buffer value"
        );

        // Quint spec doesn't go in much detail about proposal init message
        assert_eq!(
            actual.init_info.is_some(),
            expected.state.init_message.is_some(),
            "unexpected init info value"
        );

        assert!(
            utils::messages_equal_sequences(&actual.seen_sequences, &expected.state.received),
            "unexpected seen sequences value"
        );

        assert_eq!(
            actual.next_sequence, expected.state.next_sequence as u64,
            "unexpected next sequence value"
        );

        assert_eq!(
            actual.total_messages as i32, expected.state.total_messages,
            "unexpected total messages value"
        );

        assert_eq!(
            actual.fin_received, expected.state.fin_received,
            "unexpected fin received value"
        );

        assert_eq!(
            actual.emitted_messages,
            expected.state.emitted.len(),
            "unexpected emmited messages value"
        );

        Ok(true)
    }
}
