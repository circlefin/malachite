/// Process a message and handle the emitted effects.
///
/// # Example
///
/// ```rust,ignore
///
/// malachite_consensus::process!(
///     // Message to process
///     msg: msg,
///     // Consensus state and metrics
///     state: &mut state, metrics: &metrics,
///    // Effect handler
///     on: effect => handle_effect(myself, &mut timers, &mut timeouts, effect).await
/// )
/// ```
#[macro_export]
macro_rules! process {
    (msg: $msg:expr, ctx: $ctx:expr, state: $state:expr, metrics: $metrics:expr, with: $effect:ident => $handle:expr) => {{
        let mut gen =
            $crate::gen::Gen::new(|co| $crate::handle::handle(co, $ctx, $state, $metrics, $msg));

        let mut co_result = gen.resume_with(());

        loop {
            match co_result {
                $crate::gen::CoResult::Yielded($effect) => {
                    let resume = match $handle {
                        Ok(()) => (),
                        Err(error) => error!("Error when processing effect: {error:?}"),
                    };
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
macro_rules! perform {
    ($co:expr, $effect:expr) => {
        $co.yield_($effect).await
    };
}
