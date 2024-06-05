/// Whether or not a proposal is valid.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Validity {
    /// The proposal is valid.
    Valid,
    /// The proposal is invalid.
    Invalid,
}

impl Validity {
    /// Returns `true` if the proposal is valid.
    pub fn is_valid(self) -> bool {
        self == Validity::Valid
    }

    /// Returns `Valid` if given true, `Invalid` if given false.
    pub fn from_valid(valid: bool) -> Self {
        if valid {
            Validity::Valid
        } else {
            Validity::Invalid
        }
    }
}
