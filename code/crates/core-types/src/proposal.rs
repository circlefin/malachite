use core::fmt::Debug;

use crate::{Context, Round};

/// Defines the requirements for a proposal type.
///
/// # Equality and write-ahead log replay
///
/// `Eq` must distinguish proposals only by content that consensus re-derives deterministically.
/// Replaying the write-ahead log re-derives every proposal this node made before a crash.
/// The re-derived proposal and the recorded one are compared (`Eq`) and they must be equal.
/// Otherwise, it is considered a non-determinism bug.
pub trait Proposal<Ctx>
where
    Self: Clone + Debug + Eq + Send + Sync + 'static,
    Ctx: Context,
{
    /// The height for which the proposal is for.
    fn height(&self) -> Ctx::Height;

    /// The round for which the proposal is for.
    fn round(&self) -> Round;

    /// The value that is proposed.
    fn value(&self) -> &Ctx::Value;

    /// The value that is proposed.
    fn take_value(self) -> Ctx::Value;

    /// The POL round for which the proposal is for.
    fn pol_round(&self) -> Round;

    /// Address of the validator who issued this proposal
    fn validator_address(&self) -> &Ctx::Address;
}

/// Whether or not a proposal is valid.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "borsh",
    derive(::borsh::BorshSerialize, ::borsh::BorshDeserialize)
)]
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

    /// Returns `true` if the proposal is invalid.
    pub fn is_invalid(self) -> bool {
        self == Validity::Invalid
    }

    /// Converts the validity to a boolean:
    /// `true` if the proposal is valid, `false` otherwise.
    pub fn to_bool(self) -> bool {
        self.is_valid()
    }

    /// Returns `Valid` if given true, `Invalid` if given false.
    pub fn from_bool(valid: bool) -> Self {
        if valid {
            Validity::Valid
        } else {
            Validity::Invalid
        }
    }
}
