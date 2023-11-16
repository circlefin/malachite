use std::fmt::Debug;

pub trait TraceRunner {
    type State;
    type Result;

    type ExpectedState: Debug;
    type Error: Debug;

    fn init(&mut self, expected: &Self::ExpectedState) -> Result<Self::State, Self::Error>;

    fn step(
        &mut self,
        state: &mut Self::State,
        expected_state: &Self::ExpectedState,
    ) -> Result<Self::Result, Self::Error>;

    fn result_invariant(
        &self,
        result: &Self::Result,
        expected_state: &Self::ExpectedState,
    ) -> Result<bool, Self::Error>;

    fn state_invariant(
        &self,
        state: &Self::State,
        expected_state: &Self::ExpectedState,
    ) -> Result<bool, Self::Error>;

    fn test(&mut self, trace: &[Self::ExpectedState]) -> Result<(), Self::Error> {
        if let Some(init_state) = trace.first() {
            println!("🟢 step: initial");
            let mut sut_state = self.init(init_state)?;
            assert!(
                self.state_invariant(&sut_state, init_state)?,
                "🔴 state invariant failed at initialization"
            );
            for (i, state) in trace.iter().enumerate().skip(1) {
                println!("🟢 step: {}", i);
                let result = self.step(&mut sut_state, state)?;
                assert!(
                    self.result_invariant(&result, state)?,
                    "🔴 result invariant failed at step {}",
                    i
                );
                assert!(
                    self.state_invariant(&sut_state, state)?,
                    "🔴 state invariant failed at step {}",
                    i
                );
            }
        }

        Ok(())
    }
}
