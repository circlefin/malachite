use malachite_common::Context;

/// Client for use by the [`Driver`](crate::Driver) to ask
/// for a value to propose and validate proposals.
pub trait Client<Ctx>
where
    Ctx: Context,
{
    /// Get the value to propose.
    fn get_value(&self) -> Ctx::Value;

    /// Validate a proposal.
    fn validate_proposal(&self, proposal: &Ctx::Proposal) -> bool;
}
