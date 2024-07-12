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
    (msg: $msg:expr, state: $state:expr, metrics: $metrics:expr, with: $effect:ident => $handle:expr) => {{
        let mut co = Co::new(|yielder, start| {
            debug_assert!(matches!(start, Resume::Start));
            $crate::handle_msg($state, $metrics, yielder, $msg)
        });

        let mut co_result = co.resume(Resume::Start);

        loop {
            match co_result {
                CoResult::Yield($effect) => {
                    let resume = match $handle {
                        Ok(resume) => resume,
                        Err(error) => {
                            error!("Error when processing effect: {error:?}");
                            Resume::Continue
                        }
                    };
                    co_result = co.resume(resume)
                }
                CoResult::Return(result) => return result.map_err(Into::into),
            }
        }
    }};
}

/// Yield an effect and continue execution.
///
/// Effect emitted by this macro must resume with [`Resume::Continue`][continue].
///
/// # Errors
/// This macro will abort the current function with a [`Error::UnexpectedResume`][error] error
/// if the effect does not resume with [`Resume::Continue`][continue]
///
/// # Example
/// ```rust,ignore
/// let () = emit!(yielder, effect);
/// ```
///
/// [continue]: crate::effect::Resume::Continue
/// [error]: crate::error::Error::UnexpectedResume
#[macro_export]
macro_rules! emit {
    ($yielder:expr, $effect:expr) => {
        emit_then!($yielder, $effect, $crate::effect::Resume::Continue)
    };
}

/// Yield an effect, expecting a specific type of resume value.
///
/// Effects emitted by this macro must resume with a value that matches the provided pattern.
///
/// # Errors
/// This macro will abort the current function with a [`Error::UnexpectedResume`][error] error
/// if the effect does not resume with a value that matches the provided pattern.
///
/// # Example
/// ```rust,ignore
/// // If we do not need to extract the resume value
/// let () = emit_then!(yielder, effect, Resume::ProposeValue(_, _));
///
/// /// If we need to extract the resume value
/// let value: Ctx::Value = emit_then!(yielder, effect, Resume::ProposeValue(_, value) => value);
/// ```
///
/// [error]: crate::error::Error::UnexpectedResume
#[macro_export]
macro_rules! emit_then {
    ($yielder:expr, $effect:expr, $pat:pat) => {
        emit_then!($yielder, $effect, $pat => ())
    };

    // TODO: Add support for multiple patterns + if guards
    ($yielder:expr, $effect:expr, $pat:pat => $expr:expr $(,)?) => {
        match $yielder.suspend($effect) {
            $pat => $expr,
            resume => {
                return Err($crate::error::Error::UnexpectedResume(
                    resume,
                    stringify!($pat)
                )
                .into())
            }
        }
    };
}
