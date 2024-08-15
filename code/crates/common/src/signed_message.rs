use derive_where::derive_where;

use crate::{Context, Signature};

/// A signed message, ie. a message emitted by a validator and signed by its private key.
#[derive_where(Clone, Debug, PartialEq, Eq; Msg)]
pub struct SignedMessage<Ctx, Msg>
where
    Ctx: Context,
{
    /// The message
    pub message: Msg,

    /// The signature of the proposal.
    pub signature: Signature<Ctx>,
}

impl<Ctx, Msg> SignedMessage<Ctx, Msg>
where
    Ctx: Context,
{
    /// Create a new signed message from the given message and signature.
    pub fn new(message: Msg, signature: Signature<Ctx>) -> Self {
        Self { message, signature }
    }
}
