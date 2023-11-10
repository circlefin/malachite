use alloc::boxed::Box;

use async_trait::async_trait;

use malachite_common::Context;

/// Client for use by the [`Driver`](crate::Driver) to ask
/// for a value to propose and validate proposals.
#[async_trait]
pub trait Client<Ctx>
where
    Ctx: Context,
{
    /// Get the value to propose.
    async fn get_value(&self) -> Ctx::Value;

    /// Validate a proposal.
    async fn validate_proposal(&self, proposal: &Ctx::Proposal) -> bool;
}
