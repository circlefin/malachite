use crate::types::context::MockContext;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProposalPart {}

impl malachitebft_core_types::ProposalPart<MockContext> for ProposalPart {
    fn is_first(&self) -> bool {
        true
    }

    fn is_last(&self) -> bool {
        true
    }
}
