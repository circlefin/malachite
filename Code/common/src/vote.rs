use core::fmt::Debug;

use crate::{Consensus, Round, Value};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum VoteType {
    Prevote,
    Precommit,
}

pub trait Vote<C: Consensus>
where
    Self: Clone + Debug + PartialEq + Eq,
{
    fn round(&self) -> Round;
    fn value(&self) -> Option<&<C::Value as Value>::Id>;
    fn vote_type(&self) -> VoteType;

    // FIXME: round message votes should not include address
    fn address(&self) -> &C::Address;
    fn set_address(&mut self, address: C::Address);
}

pub mod test {
    use crate::test::{Address, TestConsensus, ValueId};
    use crate::Round;

    use super::VoteType;

    /// A vote for a value in a round
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Vote {
        pub typ: VoteType,
        pub round: Round,
        pub value: Option<ValueId>,
        pub address: Address,
    }

    impl Vote {
        pub fn new_prevote(round: Round, value: Option<ValueId>, address: Address) -> Self {
            Self {
                typ: VoteType::Prevote,
                round,
                value,
                address,
            }
        }

        pub fn new_precommit(round: Round, value: Option<ValueId>, address: Address) -> Self {
            Self {
                typ: VoteType::Precommit,
                round,
                value,
                address,
            }
        }
    }

    impl super::Vote<TestConsensus> for Vote {
        fn round(&self) -> Round {
            self.round
        }

        fn value(&self) -> Option<&ValueId> {
            self.value.as_ref()
        }

        fn vote_type(&self) -> VoteType {
            self.typ
        }

        fn address(&self) -> &Address {
            &self.address
        }

        fn set_address(&mut self, address: Address) {
            self.address = address;
        }
    }
}
