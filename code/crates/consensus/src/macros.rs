#[macro_export]
macro_rules! emit {
    ($yielder:expr, $effect:expr) => {
        emit_then!($yielder, $effect, $crate::handle::Resume::Continue)
    };
}

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
