use core::fmt::Debug;

// TODO: Keep the trait or just add the bounds to Consensus::Height?
pub trait Height
where
    // TODO: Require Copy as well?
    Self: Clone + Debug + PartialEq + Eq + PartialOrd + Ord,
{
}

pub mod test {
    /// A blockchain height
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
    pub struct Height(u64);

    impl Height {
        pub fn new(height: u64) -> Self {
            Self(height)
        }

        pub fn as_u64(&self) -> u64 {
            self.0
        }
    }

    impl super::Height for Height {}
}
