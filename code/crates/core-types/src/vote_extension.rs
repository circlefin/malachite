use core::fmt::Debug;

use alloc::vec::Vec;
use bytes::Bytes;
use derive_where::derive_where;

use crate::{Context, Round, SignedExtension, ValueId};

/// Policy for handling vote extensions at a given height.
///
/// The application supplies this policy as part of the height parameters. When
/// extensions are [`Disabled`](Self::Disabled), every non-nil precommit must
/// omit its extension. When they are [`Required`](Self::Required), every
/// non-nil precommit must carry a valid extension.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VoteExtensionPolicy {
    /// Vote extensions must be absent for this height.
    #[default]
    Disabled,

    /// Vote extensions must be present for this height.
    Required,
}

impl VoteExtensionPolicy {
    /// Returns `true` when every applicable precommit must carry a vote extension.
    pub const fn is_required(self) -> bool {
        matches!(self, Self::Required)
    }

    /// Returns `true` when every applicable precommit must omit its vote extension.
    pub const fn is_disabled(self) -> bool {
        matches!(self, Self::Disabled)
    }
}

/// Cryptographic scope that binds a vote extension to the precommit it accompanies.
///
/// A vote extension is meaningful only in the context of a precommit vote at a
/// specific height/round/value, cast by a specific validator. Folding this scope
/// into the extension's signed preimage prevents an extension blob from being
/// replayed across heights, rounds, values, or validators: a signature produced
/// for one scope will not verify against any other.
///
/// Producers ([`Signer::sign_vote_extension`](../../../signing/trait.Signer.html#tymethod.sign_vote_extension))
/// and verifiers ([`Verifier::verify_signed_vote_extension`](../../../signing/trait.Verifier.html#tymethod.verify_signed_vote_extension))
/// must agree on the canonical encoding of this scope.
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct VoteExtensionScope<Ctx: Context> {
    /// Height of the precommit the extension is attached to.
    pub height: Ctx::Height,
    /// Round of the precommit the extension is attached to.
    pub round: Round,
    /// Value id committed to by the precommit.
    pub value_id: ValueId<Ctx>,
    /// Address of the validator that cast the precommit and is signing the extension.
    pub validator_address: Ctx::Address,
}

impl<Ctx: Context> VoteExtensionScope<Ctx> {
    /// Create a new scope from its components.
    pub fn new(
        height: Ctx::Height,
        round: Round,
        value_id: ValueId<Ctx>,
        validator_address: Ctx::Address,
    ) -> Self {
        Self {
            height,
            round,
            value_id,
            validator_address,
        }
    }
}

/// A set of vote extensions.
#[derive_where(Clone, Debug, Default, PartialEq, Eq)]
pub struct VoteExtensions<Ctx: Context> {
    /// The vote extensions together with the address of their proposer.
    pub extensions: Vec<(Ctx::Address, SignedExtension<Ctx>)>,
}

impl<Ctx: Context> VoteExtensions<Ctx> {
    /// Creates a new set of vote extensions.
    pub fn new(mut extensions: Vec<(Ctx::Address, SignedExtension<Ctx>)>) -> Self {
        // Sort vote extensions by their proposer's address
        extensions.sort_by(|(a, _), (b, _)| a.cmp(b));

        Self { extensions }
    }

    /// Returns the size of the extensions in bytes.
    pub fn size_bytes(&self) -> usize {
        self.extensions.iter().map(|(_, e)| e.size_bytes()).sum()
    }
}

/// Vote extensions allows applications to extend the pre-commit vote with arbitrary data.
/// This allows applications to force their validators to do more than just validate blocks within consensus.
pub trait Extension
where
    Self: Clone + Debug + Eq + Send + Sync + 'static,
{
    /// Returns the size of the extension in bytes.
    fn size_bytes(&self) -> usize;
}

impl Extension for () {
    fn size_bytes(&self) -> usize {
        0
    }
}

impl Extension for Vec<u8> {
    fn size_bytes(&self) -> usize {
        self.len()
    }
}

impl Extension for Bytes {
    fn size_bytes(&self) -> usize {
        self.len()
    }
}

impl<const N: usize> Extension for [u8; N] {
    fn size_bytes(&self) -> usize {
        N
    }
}
