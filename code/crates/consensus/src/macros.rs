/// Process an input and handle the emitted effects.
///
/// # Example
///
/// ```rust,ignore
/// malachite_consensus::process!(
///     // Input to process
///     input: input,
///     // Consensus state
///     state: &mut state,
///     // Metrics
///     metrics: &metrics,
///    // Effect handler
///     on: effect => handle_effect(effect).await
/// )
/// ```
#[macro_export]
macro_rules! process {
    (input: $input:expr, state: $state:expr, metrics: $metrics:expr, with: $effect:ident => $handle:expr) => {{
        let mut gen = $crate::gen::Gen::new(|co| $crate::handle(co, $state, $metrics, $input));

        let mut co_result = gen.resume_with(());

        loop {
            match co_result {
                $crate::gen::CoResult::Yielded($effect) => {
                    if let Err(e) = $handle {
                        error!("Error when processing effect: {e:?}");
                    }

                    co_result = gen.resume_with(())
                }
                $crate::gen::CoResult::Complete(result) => {
                    return result.map_err(Into::into);
                }
            }
        }
    }};
}

/// Yield an effect, and resume the current computation after the effect has been handled.
#[macro_export]
#[doc(hidden)]
macro_rules! perform {
    ($co:expr, $effect:expr) => {
        $co.yield_($effect).await
    };
}
