use core::ops::Deref;

use derive_where::derive_where;

use crate::{Context, Signature};

/// A signed message, ie. a message emitted by a validator and signed by its private key.
#[derive_where(Clone, Debug, PartialEq, Eq, PartialOrd, Ord; Msg)]
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

    /// Map the message to a new message.
    pub fn map<F, NewMsg>(self, f: F) -> SignedMessage<Ctx, NewMsg>
    where
        F: FnOnce(Msg) -> NewMsg,
    {
        SignedMessage {
            message: f(self.message),
            signature: self.signature,
        }
    }

    /// Return a reference to the signed message.
    pub fn as_ref(&self) -> SignedMessage<Ctx, &Msg> {
        SignedMessage {
            message: &self.message,
            signature: self.signature.clone(),
        }
    }
}

impl<Ctx, Msg> Deref for SignedMessage<Ctx, Msg>
where
    Ctx: Context,
{
    type Target = Msg;

    fn deref(&self) -> &Self::Target {
        &self.message
    }
}
