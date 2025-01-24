use core::fmt::{Debug, Display};

/// Represents either `Nil` or a value of type `Value`.
///
/// This type is isomorphic to `Option<Value>` but is more explicit about its intent.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
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

/// The `Value` type denotes the value `v` carried by the `Proposal`
/// consensus message that is gossiped to other nodes by the proposer.
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
    /// The proposer publishes a `Proposal` message carrying the full value `v`, and does not stream any proposal parts at all.
    /// Better suited for small blocks to avoid overhead of gossiping parts.
    /// In this case `Value` is typically set to be the block and `Id` is its hash.
    ProposalOnly,

    /// The proposer does not publish a `Proposal` message at all, it only streams the proposed value as proposal parts.
    /// In this case `Value` is typically set to the same type as `Id`.
    PartsOnly,

    /// The proposer publishes a `Proposal` message carrying only `id(v)`, and streams the full value as proposal parts.
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
