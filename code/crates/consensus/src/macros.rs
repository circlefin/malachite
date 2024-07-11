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

    // TODO: Add support for if guards
    ($yielder:expr, $effect:expr $(, $pat:pat => $expr:expr)+ $(,)*) => {
        match $yielder.suspend($effect) {
            $($pat => $expr,)+
            resume => {
                return Err($crate::error::Error::UnexpectedResume(
                    resume,
                    concat!(concat!($(stringify!($pat))+), ", ")
                )
                .into())
            }
        }
    };
}
