use core::fmt;

use alloc::boxed::Box;

/// A boxed error type which implements `Error`, `Send`, `Sync`, `PartialEq` and `Display`.
#[derive(Debug)]
pub struct BoxError(pub Box<dyn core::error::Error + Send + Sync>);

impl BoxError {
    /// Create a new `BoxError` from a boxed error.
    pub fn new(error: Box<dyn core::error::Error + Send + Sync>) -> Self {
        Self(error)
    }

    /// Get the underlying error.
    pub fn into_inner(self) -> Box<dyn core::error::Error + Send + Sync> {
        self.0
    }
}

impl PartialEq for BoxError {
    fn eq(&self, _other: &Self) -> bool {
        // We don't compare the contents of the error, just that it is an error
        true
    }
}

impl fmt::Display for BoxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl core::error::Error for BoxError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        self.0.source()
    }
}

impl From<Box<dyn core::error::Error + Send + Sync>> for BoxError {
    fn from(error: Box<dyn core::error::Error + Send + Sync>) -> Self {
        BoxError::new(error)
    }
}
