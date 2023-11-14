use alloc::boxed::Box;

use async_trait::async_trait;

use malachite_common::{Context, Round};

/// Environment for use by the [`Driver`](crate::Driver) to ask
/// for a value to propose and validate proposals.
#[async_trait]
pub trait Env<Ctx>
where
    Ctx: Context,
{
    /// Get the value to propose for the given height and round.
    ///
    /// If `None` is returned, the driver will understand this
    /// as an error and will not propose a value.
    async fn get_value(&self, height: Ctx::Height, round: Round) -> Option<Ctx::Value>;
}
