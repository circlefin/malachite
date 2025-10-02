use alloc::boxed::Box;
use core::fmt;
use malachitebft_core_types::BoxError;

/// Much like in the `signature` crate, this type is deliberately opaque as to avoid sidechannel
/// leakage which could potentially be used recover signing private keys or forge signatures (e.g. BB’06).
///
/// It impls `core::error::Error` and supports an optional `core::error::Error::source`,
/// which can be used by things like remote signers (e.g. HSM, KMS) to report I/O or auth errors.
#[derive(Default, Debug, PartialEq)]
#[non_exhaustive]
pub struct Error {
    source: Option<BoxError>,
}

impl Error {
    /// Create a new error with no associated source
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new error with an associated source.
    ///
    /// **NOTE:** The "source" should **NOT** be used to propagate cryptographic
    /// errors e.g. signature parsing or verification errors. The intended use
    /// cases are for propagating errors related to external signers, e.g.
    /// communication/authentication errors with HSMs, KMS, etc.
    pub fn from_source(source: impl Into<Box<dyn core::error::Error + Send + Sync>>) -> Self {
        Self {
            source: Some(BoxError::from(source.into())),
        }
    }

    pub fn into_source(self) -> Option<BoxError> {
        self.source
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("signature error")?;

        if let Some(source) = &self.source {
            write!(f, ": {}", source)?;
        }

        Ok(())
    }
}

impl From<Box<dyn core::error::Error + Send + Sync + 'static>> for Error {
    fn from(source: Box<dyn core::error::Error + Send + Sync + 'static>) -> Error {
        Self::from_source(source)
    }
}

impl core::error::Error for Error {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        self.source
            .as_ref()
            .map(|source| source.0.as_ref() as &(dyn core::error::Error + 'static))
    }
}
