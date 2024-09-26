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

/// Yield an effect, expecting a specific type of resume value.
///
/// Effects yielded by this macro must resume with a value that matches the provided pattern.
/// If not pattern is give, then the yielded effect must resume with [`Resume::Continue`][continue].
///
/// # Errors
/// This macro will abort the current function with a [`Error::UnexpectedResume`][error] error
/// if the effect does not resume with a value that matches the provided pattern.
///
/// # Example
/// ```rust,ignore
/// // If we do not need to extract the resume value
/// let () = perform!(co, effect, Resume::ProposeValue(_, _));
///
/// /// If we need to extract the resume value
/// let value: Ctx::Value = perform!(co, effect, Resume::ProposeValue(_, value) => value);
/// ```
///
/// [error]: crate::error::Error::UnexpectedResume
#[macro_export]
macro_rules! perform {
    ($co:expr, $effect:expr) => {
        $co.yield_($effect).await
    };
}
