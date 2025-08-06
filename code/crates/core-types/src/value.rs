use core::fmt::{Debug, Display};

/// Represents either `Nil` or a value of type `Value`.
///
/// This type is isomorphic to `Option<Value>` but is more explicit about its intent.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub enum NilOrVal<Value> {
    /// The value is `nil`.
    #[default]
    Nil,

    /// The value is a value of type `Value`.
    Val(Value),
}

impl<Value> NilOrVal<Value> {
    /// Whether this is `nil`.
    pub fn is_nil(&self) -> bool {
        matches!(self, Self::Nil)
    }

    /// Whether this is an actual value.
    pub fn is_val(&self) -> bool {
        matches!(self, Self::Val(_))
    }

    /// Apply the given function to the value if it is not `nil`.
    pub fn map<NewValue, F: FnOnce(Value) -> NewValue>(self, f: F) -> NilOrVal<NewValue> {
        match self {
            NilOrVal::Nil => NilOrVal::Nil,
            NilOrVal::Val(value) => NilOrVal::Val(f(value)),
        }
    }

    /// Convert this into an `NilOrVal<&Value>`, allowing to borrow the value.
    pub fn as_ref(&self) -> NilOrVal<&Value> {
        match self {
            NilOrVal::Nil => NilOrVal::Nil,
            NilOrVal::Val(value) => NilOrVal::Val(value),
        }
    }

    /// Consumes this and returns the value if it is not `nil`,
    /// otherwise returns the default `Value`.
    // (note adi) Find what is this for? Could not find a way to use it.
    pub fn value_or_default(self) -> Value
    where
        Value: Default,
    {
        match self {
            NilOrVal::Nil => Value::default(),
            NilOrVal::Val(value) => value,
        }
    }
}

impl<Value> NilOrVal<&Value> {
    /// Clone the underlying value
    #[must_use = "`self` will be dropped if the result is not used"]
    pub fn cloned(self) -> NilOrVal<Value>
    where
        Value: Clone,
    {
        match self {
            NilOrVal::Nil => NilOrVal::Nil,
            NilOrVal::Val(value) => NilOrVal::Val(value.clone()),
        }
    }
}

/// The `Value` type denotes the value `v` carried by the `Proposal`
/// consensus message broadcast by the proposer of a round of consensus.
///
/// How to instantiate `Value` with a concrete type depends on which mode consensus
/// is parametrized to run in. See the documentation for the [`ValuePayload`]
/// type for more information.
pub trait Value
where
    Self: Clone + Debug + PartialEq + Eq + PartialOrd + Ord + Send + Sync,
{
    /// A unique representation of the `Value` with a lower memory footprint, denoted `id(v)`.
    /// It is carried by votes and herefore is typically set to be a hash of the value `v`.
    type Id: Clone + Debug + Display + Eq + Ord + Send + Sync;

    /// The ID of the value.
    fn id(&self) -> Self::Id;
}

/// The possible messages used to deliver proposals
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValuePayload {
    /// The proposer broadcasts a `Proposal` consensus message carrying the full proposed value `v`. There is no proposal part streaming.
    /// Better suited for small proposed values when there are no benefits of gossiping proposal parts.
    /// In this case `Value` is typically set to be the block and `Id` is its hash.
    ProposalOnly,

    /// The proposer does not broadcast a `Proposal` consensus message at all. The proposer only streams the proposed value as proposal parts.
    /// In this case `Value` is typically set to the same type as `Id`.
    PartsOnly,

    /// The proposer broadcasts a `Proposal` message carrying `id(v)` and streams the full proposed value `v` as proposal parts.
    /// In this case `Value` is typically set to the same type as `Id`.
    ProposalAndParts,
}

impl ValuePayload {
    /// Whether the proposer must publish the proposed value as a `Proposal` message.
    pub fn include_proposal(self) -> bool {
        matches!(self, Self::ProposalOnly | Self::ProposalAndParts)
    }

    /// Whether the proposer must publish the proposed value as parts.
    pub fn include_parts(self) -> bool {
        matches!(self, Self::PartsOnly | Self::ProposalAndParts)
    }

    /// Whether the proposal must only publish proposal parts, no `Proposal` message.
    pub fn parts_only(self) -> bool {
        matches!(self, Self::PartsOnly)
    }

    /// Whether the proposer must only publish a `Proposal` message, no proposal parts.
    pub fn proposal_only(&self) -> bool {
        matches!(self, Self::ProposalOnly)
    }
}

/// Protocols that diseminate `Value`
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ValueOrigin {
    /// Synchronization protocol
    Sync,

    /// Consensus protocol
    Consensus,
}

impl ValueOrigin {
    /// Value was received from the synchronization protocol.
    pub fn is_sync(&self) -> bool {
        matches!(self, Self::Sync)
    }

    /// Value was received from the consensus protocol.
    pub fn is_consensus(&self) -> bool {
        matches!(self, Self::Consensus)
    }
}
