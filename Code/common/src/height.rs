// TODO: Abstract over Height

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
