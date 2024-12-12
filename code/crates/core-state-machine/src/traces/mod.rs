mod line;

#[allow(unused_imports)]
pub use line::Line;

#[cfg(feature = "debug")]
mod trace;

#[cfg(feature = "debug")]
pub use trace::Trace;

#[doc(hidden)]
#[macro_export]
macro_rules! debug_trace {
    ($state:expr, $line:expr) => {
        #[cfg(feature = "debug")]
        {
            #[allow(unused_imports)]
            use $crate::traces::Line;

            $state.add_trace($line);
        }
        #[cfg(not(feature = "debug"))]
        {
            let _ = &mut $state;
        }
    };
}
